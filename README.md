# Library borrowing, as one complete story

An active member borrows one exact available book. The operation records either the loan or a business refusal once. Replaying the same operation returns that recorded result. If the write settles but its acknowledgement is lost, recovery finds the result without creating another loan.

The public call is deliberately boring:

```rust
use usat_library_demo::{
    BookId, BorrowCommand, Library, MemberId, OperationId, borrow_book,
};

let mut library = Library::new();
library.add_member(MemberId(7), true);
library.add_book(BookId(42), true);

let command = BorrowCommand::new(OperationId(1001), MemberId(7), BookId(42));
let outcome = borrow_book(&library, command).unwrap();
println!("{outcome:?}");
```

Inside `borrow_book`, private carriers move through:

```text
Opened → Authorized/Refused → Prepared → Settled
```

The command binds operation, member, and book. Each stage retains that command and the same locked state; later stages accept no replacement IDs. Admission checks the actual member and book. Settlement stores the outcome under the operation ID before an acknowledgement can be lost.

What this example guarantees:

- inactive members and missing or unavailable books produce recorded refusals;
- one operation ID cannot be reused for another member or book;
- replay returns the first outcome without another loan;
- policy changes do not rewrite a settled refusal;
- recovery matches the original command before returning its settlement;
- callers cannot construct, skip, retarget, or reuse private stages. Compile-fail doctests check those boundaries.

Run every check with:

```sh
cargo fmt --check
cargo clippy --all-targets -- -D warnings
cargo test
```

`Library` is an in-memory teaching fixture. Its mutex gives one process a short exclusive critical section. It has no disk storage, authentication, crash recovery, multi-process locking, or distributed transaction guarantee. A production version would keep the same story boundary but put intent, loans, refusals, and operation uniqueness in durable transactional storage.
