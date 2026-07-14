//! Match primitives: the palette's fuzzy scorer and the case/diacritic
//! folding helpers shared with `/` search.


/// Fuzzy-match `query` against `text` (the file palette's matcher). Returns a
/// relevance score if every `query` character appears in `text` in order
/// (a subsequence match, case-insensitive over ASCII), else `None`. Higher is
/// better; a higher score is a "tighter" match.
///
/// Scoring rewards two things prose filenames make meaningful: a match at a
/// **word boundary** (start of string, or just after `/ _ - . space`) scores far
/// above one mid-word, and a **run** of consecutive matches scores extra per
/// char. So typing `notes` ranks `repo/notes.md` above `promo-tests.md` even
/// though both contain the letters. There are no penalties, so a score is always
/// ≥ the query length; ties are broken by the caller (recency, then list order).
/// An empty query matches everything with score 0.
///
/// A **space** in the query matches any separator (`/ _ - . space`), so typing
/// `la conv` finds `la-convergence.md` — filenames use `-` where the typist
/// thinks "space".
pub(crate) fn fuzzy_score(query: &str, text: &str) -> Option<i32> {
    let q: Vec<char> = query.chars().collect();
    if q.is_empty() {
        return Some(0);
    }
    let mut qi = 0;
    let mut score = 0i32;
    let mut prev_matched = false;
    let mut prev: Option<char> = None;
    for (i, tc) in text.chars().enumerate() {
        let hit = qi < q.len()
            && (tc.eq_ignore_ascii_case(&q[qi])
                || (q[qi] == ' ' && matches!(tc, '/' | '_' | '-' | '.' | ' ')));
        if hit {
            score += 1;
            let boundary =
                i == 0 || matches!(prev, Some('/') | Some('_') | Some('-') | Some('.') | Some(' '));
            if boundary {
                score += 10;
            }
            if prev_matched {
                score += 5;
            }
            qi += 1;
            prev_matched = true;
        } else {
            prev_matched = false;
        }
        prev = Some(tc);
    }
    (qi == q.len()).then_some(score)
}


/// Byte offset of the first folded match of `pat` in `hay`, or `None`.
/// Char-by-char comparison through [`fold`] at every char boundary — no
/// folded copy of the buffer (folding can change byte lengths, which would
/// break the returned offsets). `ci` is the smartcase verdict, computed once
/// per search from the pattern. O(n·m), fine at note sizes for an
/// Enter-triggered jump.
pub(crate) fn find_fold(hay: &str, pat: &str, ci: bool) -> Option<usize> {
    hay.char_indices()
        .map(|(i, _)| i)
        .find(|&i| starts_with_fold(&hay[i..], pat, ci))
}

/// [`find_fold`], but the *last* match — the backward (`N`) direction.
pub(crate) fn rfind_fold(hay: &str, pat: &str, ci: bool) -> Option<usize> {
    hay.char_indices()
        .map(|(i, _)| i)
        .rev()
        .find(|&i| starts_with_fold(&hay[i..], pat, ci))
}

/// Whether `s` begins with `pat` under [`fold`].
pub(crate) fn starts_with_fold(s: &str, pat: &str, ci: bool) -> bool {
    let mut sc = s.chars();
    pat.chars()
        .all(|p| sc.next().is_some_and(|c| fold(c, ci) == fold(p, ci)))
}

/// A char's search identity: diacritics are always stripped (`é` = `e`, so
/// `/ete` finds `été` and vice versa — accents are how the word is *spelled*,
/// not what you're *searching for*), and case is dropped only when `ci`
/// (the smartcase rule: an all-lowercase pattern searches insensitively; one
/// capital in it makes the search exact).
pub(crate) fn fold(c: char, ci: bool) -> char {
    let c = if ci {
        // First char of the lowercase expansion — 1:1 for all of Latin,
        // which is what this appliance types.
        c.to_lowercase().next().unwrap_or(c)
    } else {
        c
    };
    strip_diacritic(c)
}

/// Map accented Latin letters to their base letter, both cases (the Latin-1
/// Supplement set — the French/Western repertoire the keymap can produce).
/// Ligatures (`œ`, `æ`) fold to more than one char and are left alone.
pub(crate) fn strip_diacritic(c: char) -> char {
    match c {
        'à'..='å' => 'a',
        'ç' => 'c',
        'è'..='ë' => 'e',
        'ì'..='ï' => 'i',
        'ñ' => 'n',
        'ò'..='ö' => 'o',
        'ù'..='ü' => 'u',
        'ý' | 'ÿ' => 'y',
        'À'..='Å' => 'A',
        'Ç' => 'C',
        'È'..='Ë' => 'E',
        'Ì'..='Ï' => 'I',
        'Ñ' => 'N',
        'Ò'..='Ö' => 'O',
        'Ù'..='Ü' => 'U',
        'Ý' => 'Y',
        _ => c,
    }
}
