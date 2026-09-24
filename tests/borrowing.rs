use usat_library_demo::{
    BookId, BorrowCommand, BorrowError, BorrowOutcome, BorrowRefusal, Library, MemberId,
    OperationId, borrow_book, recover_borrow,
};

fn command(operation: u64, member: u64, book: u64) -> BorrowCommand {
    BorrowCommand::new(OperationId(operation), MemberId(member), BookId(book))
}

fn library(member_active: bool, book_available: bool) -> Library {
    let mut library = Library::new();
    library.add_member(MemberId(1), member_active);
    library.add_book(BookId(10), book_available);
    library
}

#[test]
fn active_member_borrows_exact_available_book() {
    let library = library(true, true);
    let outcome = borrow_book(&library, command(100, 1, 10)).unwrap();

    assert_eq!(
        outcome,
        BorrowOutcome::Borrowed {
            member: MemberId(1),
            book: BookId(10),
        }
    );
    assert_eq!(library.loan_count(), 1);
}

#[test]
fn inactive_member_gets_settled_refusal() {
    let library = library(false, true);

    assert_eq!(
        borrow_book(&library, command(100, 1, 10)),
        Ok(BorrowOutcome::Refused(BorrowRefusal::InactiveMember))
    );
    assert_eq!(library.loan_count(), 0);
}

#[test]
fn unavailable_book_gets_settled_refusal() {
    let library = library(true, false);

    assert_eq!(
        borrow_book(&library, command(100, 1, 10)),
        Ok(BorrowOutcome::Refused(BorrowRefusal::BookUnavailable))
    );
    assert_eq!(library.loan_count(), 0);
}

#[test]
fn missing_book_gets_settled_refusal() {
    let library = library(true, true);

    assert_eq!(
        borrow_book(&library, command(100, 1, 99)),
        Ok(BorrowOutcome::Refused(BorrowRefusal::BookMissing))
    );
    assert_eq!(library.loan_count(), 0);
}

#[test]
fn replay_returns_original_outcome_without_another_loan() {
    let library = library(true, true);
    let work = command(100, 1, 10);
    let original = borrow_book(&library, work.clone()).unwrap();

    assert_eq!(borrow_book(&library, work), Ok(original));
    assert_eq!(library.loan_count(), 1);
}

#[test]
fn operation_id_cannot_be_rebound() {
    let library = library(true, true);
    borrow_book(&library, command(100, 1, 10)).unwrap();

    assert_eq!(
        borrow_book(&library, command(100, 2, 10)),
        Err(BorrowError::OperationConflict)
    );
    assert_eq!(
        borrow_book(&library, command(100, 1, 11)),
        Err(BorrowError::OperationConflict)
    );
    assert_eq!(library.loan_count(), 1);
}

#[test]
fn settled_refusal_does_not_change_after_policy_changes() {
    let library = library(false, true);
    let work = command(100, 1, 10);
    assert_eq!(
        borrow_book(&library, work.clone()),
        Ok(BorrowOutcome::Refused(BorrowRefusal::InactiveMember))
    );

    assert!(library.set_member_active(MemberId(1), true));

    assert_eq!(
        borrow_book(&library, work),
        Ok(BorrowOutcome::Refused(BorrowRefusal::InactiveMember))
    );
    assert_eq!(library.loan_count(), 0);
}

#[test]
fn lost_acknowledgement_recovers_original_settlement() {
    let library = library(true, true);
    let work = command(100, 1, 10);
    library.lose_next_acknowledgement();

    assert_eq!(
        borrow_book(&library, work.clone()),
        Err(BorrowError::AcknowledgementUnknown)
    );
    assert_eq!(
        recover_borrow(&library, &work),
        Ok(Some(BorrowOutcome::Borrowed {
            member: MemberId(1),
            book: BookId(10),
        }))
    );
    assert_eq!(library.loan_count(), 1);
}

#[test]
fn recovery_rejects_changed_intent() {
    let library = library(true, true);
    borrow_book(&library, command(100, 1, 10)).unwrap();

    assert_eq!(
        recover_borrow(&library, &command(100, 1, 11)),
        Err(BorrowError::OperationConflict)
    );
}
