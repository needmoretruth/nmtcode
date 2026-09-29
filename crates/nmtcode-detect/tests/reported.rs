//! Which rejected candidates a reader reports (`Scan::reported_errors`).

use nmtcode_core::{Error, ModuleGrid};
use nmtcode_detect::{Found, Point, Rejected, Scan};

fn rejected(error: Error) -> Rejected {
    Rejected { corners: [Point::new(0.0, 0.0); 4], error }
}

fn found() -> Found {
    Found {
        grid: ModuleGrid::new(20, 20).unwrap(),
        uncertain: Vec::new(),
        corners: [Point::new(0.0, 0.0); 4],
        mirrored: false,
        inverted: false,
        finders: 4,
    }
}

#[test]
fn unreadable_candidates_are_reported_once_and_only_when_nothing_else_was_read() {
    let mut scan = Scan::default();
    assert!(scan.reported_errors().is_empty());
    scan.rejected = vec![rejected(Error::FormatUnreadable), rejected(Error::FormatUnreadable)];
    assert_eq!(scan.reported_errors(), [Error::FormatUnreadable]);
    scan.found = vec![found()];
    assert!(scan.reported_errors().is_empty());
    scan.found.clear();
    scan.rejected.push(rejected(Error::SizeLimit));
    assert_eq!(scan.reported_errors(), [Error::SizeLimit]);
}

#[test]
fn decoded_format_words_are_always_reported_and_a_nested_pair_once_last() {
    let scan = Scan {
        found: vec![found()],
        rejected: vec![
            rejected(Error::NestedSymbol),
            rejected(Error::FormatConflict),
            rejected(Error::NestedSymbol),
            rejected(Error::FormatVersion),
        ],
        inverted: false,
    };
    assert_eq!(
        scan.reported_errors(),
        [Error::FormatConflict, Error::FormatVersion, Error::NestedSymbol]
    );
}
