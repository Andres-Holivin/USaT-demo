//! Small USaT borrowing example. Callers get one complete operation, not stage handles.
//!
//! Later stages cannot be constructed:
//!
//! ```compile_fail
//! use usat_library_demo::Prepared;
//! ```
//!
//! Authorization cannot be skipped:
//!
//! ```compile_fail
//! use usat_library_demo::{BookId, BorrowCommand, MemberId, OperationId};
//! let command = BorrowCommand::new(OperationId(1), MemberId(1), BookId(1));
//! command.prepare();
//! ```
//!
//! Bound work cannot be retargeted:
//!
//! ```compile_fail
//! use usat_library_demo::{BookId, BorrowCommand, MemberId, OperationId};
//! let mut command = BorrowCommand::new(OperationId(1), MemberId(1), BookId(1));
//! command.book = BookId(2);
//! ```
//!
//! Consumed work cannot be reused accidentally:
//!
//! ```compile_fail
//! use usat_library_demo::{BookId, BorrowCommand, Library, MemberId, OperationId, borrow_book};
//! let library = Library::new();
//! let command = BorrowCommand::new(OperationId(1), MemberId(1), BookId(1));
//! let _ = borrow_book(&library, command);
//! let _ = borrow_book(&library, command);
//! ```

use std::collections::BTreeMap;
use std::sync::{Mutex, MutexGuard};

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct MemberId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct BookId(pub u64);

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct OperationId(pub u64);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BorrowCommand {
    operation: OperationId,
    member: MemberId,
    book: BookId,
}

impl BorrowCommand {
    pub fn new(operation: OperationId, member: MemberId, book: BookId) -> Self {
        Self {
            operation,
            member,
            book,
        }
    }

    pub fn operation(&self) -> OperationId {
        self.operation
    }

    pub fn member(&self) -> MemberId {
        self.member
    }

    pub fn book(&self) -> BookId {
        self.book
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BorrowOutcome {
    Borrowed { member: MemberId, book: BookId },
    Refused(BorrowRefusal),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BorrowRefusal {
    InactiveMember,
    BookMissing,
    BookUnavailable,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum BorrowError {
    OperationConflict,
    AcknowledgementUnknown,
}

#[derive(Clone)]
struct Settlement {
    command: BorrowCommand,
    outcome: BorrowOutcome,
}

#[derive(Default)]
struct State {
    members: BTreeMap<MemberId, bool>,
    books: BTreeMap<BookId, bool>,
    operations: BTreeMap<OperationId, Settlement>,
    loans: Vec<(MemberId, BookId)>,
    lose_next_acknowledgement: bool,
}

#[derive(Default)]
pub struct Library {
    state: Mutex<State>,
}

impl Library {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_member(&mut self, member: MemberId, active: bool) {
        self.state.get_mut().unwrap().members.insert(member, active);
    }

    pub fn add_book(&mut self, book: BookId, available: bool) {
        self.state.get_mut().unwrap().books.insert(book, available);
    }

    pub fn set_member_active(&self, member: MemberId, active: bool) -> bool {
        let mut state = self.state.lock().unwrap();
        let Some(current) = state.members.get_mut(&member) else {
            return false;
        };
        *current = active;
        true
    }

    pub fn lose_next_acknowledgement(&self) {
        self.state.lock().unwrap().lose_next_acknowledgement = true;
    }

    pub fn loan_count(&self) -> usize {
        self.state.lock().unwrap().loans.len()
    }
}

enum Resolution<'a> {
    Replay(BorrowOutcome),
    Opened(Opened<'a>),
}

struct Opened<'a> {
    command: BorrowCommand,
    state: MutexGuard<'a, State>,
}

enum Admission<'a> {
    Authorized(Authorized<'a>),
    Refused(Refused<'a>),
}

struct Authorized<'a> {
    command: BorrowCommand,
    state: MutexGuard<'a, State>,
}

struct Refused<'a> {
    command: BorrowCommand,
    reason: BorrowRefusal,
    state: MutexGuard<'a, State>,
}

struct Prepared<'a> {
    command: BorrowCommand,
    outcome: BorrowOutcome,
    state: MutexGuard<'a, State>,
}

struct Settled {
    outcome: BorrowOutcome,
    acknowledgement_lost: bool,
}

impl Library {
    fn open(&self, command: BorrowCommand) -> Result<Resolution<'_>, BorrowError> {
        let state = self.state.lock().unwrap();
        if let Some(existing) = state.operations.get(&command.operation) {
            return if existing.command == command {
                Ok(Resolution::Replay(existing.outcome.clone()))
            } else {
                Err(BorrowError::OperationConflict)
            };
        }
        Ok(Resolution::Opened(Opened { command, state }))
    }
}

impl<'a> Opened<'a> {
    fn authorize(self) -> Admission<'a> {
        let reason = match self.state.members.get(&self.command.member) {
            Some(true) => match self.state.books.get(&self.command.book) {
                None => Some(BorrowRefusal::BookMissing),
                Some(false) => Some(BorrowRefusal::BookUnavailable),
                Some(true) => None,
            },
            _ => Some(BorrowRefusal::InactiveMember),
        };

        match reason {
            Some(reason) => Admission::Refused(Refused {
                command: self.command,
                reason,
                state: self.state,
            }),
            None => Admission::Authorized(Authorized {
                command: self.command,
                state: self.state,
            }),
        }
    }
}

impl<'a> Authorized<'a> {
    fn prepare(self) -> Prepared<'a> {
        let outcome = BorrowOutcome::Borrowed {
            member: self.command.member,
            book: self.command.book,
        };
        Prepared {
            command: self.command,
            outcome,
            state: self.state,
        }
    }
}

impl<'a> Refused<'a> {
    fn prepare(self) -> Prepared<'a> {
        Prepared {
            command: self.command,
            outcome: BorrowOutcome::Refused(self.reason),
            state: self.state,
        }
    }
}

impl Prepared<'_> {
    fn settle(mut self) -> Settled {
        if let BorrowOutcome::Borrowed { member, book } = self.outcome {
            *self.state.books.get_mut(&book).unwrap() = false;
            self.state.loans.push((member, book));
        }
        self.state.operations.insert(
            self.command.operation,
            Settlement {
                command: self.command,
                outcome: self.outcome.clone(),
            },
        );
        let acknowledgement_lost = std::mem::take(&mut self.state.lose_next_acknowledgement);
        Settled {
            outcome: self.outcome,
            acknowledgement_lost,
        }
    }
}

pub fn borrow_book(
    library: &Library,
    command: BorrowCommand,
) -> Result<BorrowOutcome, BorrowError> {
    let prepared = match library.open(command)? {
        Resolution::Replay(outcome) => return Ok(outcome),
        Resolution::Opened(opened) => match opened.authorize() {
            Admission::Authorized(stage) => stage.prepare(),
            Admission::Refused(stage) => stage.prepare(),
        },
    };
    let settled = prepared.settle();
    if settled.acknowledgement_lost {
        Err(BorrowError::AcknowledgementUnknown)
    } else {
        Ok(settled.outcome)
    }
}

pub fn recover_borrow(
    library: &Library,
    command: &BorrowCommand,
) -> Result<Option<BorrowOutcome>, BorrowError> {
    let state = library.state.lock().unwrap();
    let Some(existing) = state.operations.get(&command.operation) else {
        return Ok(None);
    };
    if existing.command != *command {
        return Err(BorrowError::OperationConflict);
    }
    Ok(Some(existing.outcome.clone()))
}
