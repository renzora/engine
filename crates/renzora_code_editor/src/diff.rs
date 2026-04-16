//! Line-based diff for the diff modal.
//!
//! Uses a simple LCS over line hashes — fine for editor-sized files
//! (thousands of lines), and avoids pulling in a diffing crate just for
//! the editor's compare-two-buffers feature.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiffOp {
    /// Both sides have this line at this index.
    Same,
    /// Only the left side has this line.
    Removed,
    /// Only the right side has this line.
    Added,
}

#[derive(Debug, Clone)]
pub struct DiffLine {
    pub op: DiffOp,
    /// Left-side line text (None when this row is an Added line).
    pub left: Option<String>,
    /// Right-side line text (None when this row is a Removed line).
    pub right: Option<String>,
    /// Left line number (1-based) if applicable.
    pub left_lineno: Option<usize>,
    /// Right line number (1-based) if applicable.
    pub right_lineno: Option<usize>,
}

/// Compute a line-by-line diff. Returns paired rows, suitable for displaying
/// side-by-side or as a unified view.
pub fn diff_lines(left: &str, right: &str) -> Vec<DiffLine> {
    let l: Vec<&str> = left.split('\n').collect();
    let r: Vec<&str> = right.split('\n').collect();

    // LCS table (rows = left, cols = right).
    let m = l.len();
    let n = r.len();
    let mut dp = vec![vec![0u32; n + 1]; m + 1];
    for i in 0..m {
        for j in 0..n {
            if l[i] == r[j] {
                dp[i + 1][j + 1] = dp[i][j] + 1;
            } else {
                dp[i + 1][j + 1] = dp[i + 1][j].max(dp[i][j + 1]);
            }
        }
    }

    // Backtrack to build the diff.
    let mut out = Vec::new();
    let mut i = m;
    let mut j = n;
    while i > 0 && j > 0 {
        if l[i - 1] == r[j - 1] {
            out.push(DiffLine {
                op: DiffOp::Same,
                left: Some(l[i - 1].to_string()),
                right: Some(r[j - 1].to_string()),
                left_lineno: Some(i),
                right_lineno: Some(j),
            });
            i -= 1;
            j -= 1;
        } else if dp[i - 1][j] >= dp[i][j - 1] {
            out.push(DiffLine {
                op: DiffOp::Removed,
                left: Some(l[i - 1].to_string()),
                right: None,
                left_lineno: Some(i),
                right_lineno: None,
            });
            i -= 1;
        } else {
            out.push(DiffLine {
                op: DiffOp::Added,
                left: None,
                right: Some(r[j - 1].to_string()),
                left_lineno: None,
                right_lineno: Some(j),
            });
            j -= 1;
        }
    }
    while i > 0 {
        out.push(DiffLine {
            op: DiffOp::Removed,
            left: Some(l[i - 1].to_string()),
            right: None,
            left_lineno: Some(i),
            right_lineno: None,
        });
        i -= 1;
    }
    while j > 0 {
        out.push(DiffLine {
            op: DiffOp::Added,
            left: None,
            right: Some(r[j - 1].to_string()),
            left_lineno: None,
            right_lineno: Some(j),
        });
        j -= 1;
    }

    out.reverse();
    out
}

/// True if every row is `Same` — a quick "files are identical" probe.
pub fn is_identical(rows: &[DiffLine]) -> bool {
    rows.iter().all(|r| r.op == DiffOp::Same)
}
