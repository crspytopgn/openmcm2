//! Keeping rude words out of names other players see: shared tracks and
//! their makers, leaderboard riders and races in the server browser. The
//! server does the same (`tools/leaderboard/worker.js`, the same list),
//! and the game masks what it shows as well.
//!
//! The list is `tools/leaderboard/badwords.txt`: the List of Dirty,
//! Naughty, Obscene, and Otherwise Bad Words (LDNOOBW), CC BY 4.0. A word
//! is masked when it is a listed word (with an s, es, ed, er, ers, ing or
//! y on the end too), read past numbers standing in for letters (5h1t),
//! and a listed phrase or a word spelled a letter at a time is masked
//! whole. Parts of ordinary words (Scunthorpe, Sussex, Therapist) are
//! left alone.

use std::sync::OnceLock;

const LIST: &str = include_str!("../../../tools/leaderboard/badwords.txt");

struct Words {
    /// Single words.
    words: std::collections::HashSet<String>,
    /// Phrases (and words), their letters run together.
    joined: std::collections::HashSet<String>,
}

fn words() -> &'static Words {
    static W: OnceLock<Words> = OnceLock::new();
    W.get_or_init(|| {
        let mut words = std::collections::HashSet::new();
        let mut joined = std::collections::HashSet::new();
        for l in LIST.lines().map(str::trim).filter(|l| !l.is_empty() && !l.starts_with('#')) {
            let parts: Vec<String> = l.split_whitespace().map(fold).collect();
            if parts.len() == 1 {
                words.insert(parts[0].clone());
            }
            joined.insert(parts.concat());
        }
        Words { words, joined }
    })
}

/// Lower case, numbers and signs that stand in for letters read as them,
/// anything else not a letter dropped.
fn fold(s: &str) -> String {
    s.chars()
        .filter_map(|c| {
            let c = c.to_ascii_lowercase();
            Some(match c {
                '0' => 'o',
                '1' | '!' | '|' => 'i',
                '3' => 'e',
                '4' | '@' => 'a',
                '5' | '$' => 's',
                '7' => 't',
                '8' => 'b',
                c if c.is_alphabetic() || c == '&' => c,
                _ => return None,
            })
        })
        .collect()
}

/// Words rude wherever they are in a longer word ("shithead"), and never
/// part of an ordinary one.
const ANYWHERE: [&str; 6] = ["fuck", "shit", "wank", "nigger", "nigga", "faggot"];

fn bad_word(w: &str) -> bool {
    let set = &words().words;
    if w.is_empty() {
        return false;
    }
    if set.contains(w) || ANYWHERE.iter().any(|a| w.contains(a)) {
        return true;
    }
    ["s", "es", "ed", "er", "ers", "ing", "y"].iter().any(|end| w.strip_suffix(end).is_some_and(|stem| stem.len() >= 3 && set.contains(stem)))
}

/// Whether the name has a rude word in it.
pub fn is_rude(name: &str) -> bool {
    mask(name) != name
}

/// The name with each rude word starred out but its first letter
/// ("f***"). A listed phrase ("blow job") or a word spelled out a letter
/// at a time ("f u c k") is starred whole.
pub fn mask(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let is_part = |c: char| c.is_alphanumeric() || matches!(c, '@' | '$' | '!' | '&');
    // The words: where each starts and ends.
    let mut spans: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        if is_part(chars[i]) {
            let a = i;
            while i < chars.len() && is_part(chars[i]) {
                i += 1;
            }
            spans.push((a, i));
        } else {
            i += 1;
        }
    }
    let text = |k: usize| -> String { chars[spans[k].0..spans[k].1].iter().collect() };
    let folded: Vec<String> = (0..spans.len()).map(|k| fold(&text(k))).collect();
    let mut bad = vec![false; spans.len()];
    let mut whole = vec![false; spans.len()];
    for k in 0..spans.len() {
        bad[k] = bad_word(&folded[k]);
    }
    let w = words();
    for k in 0..spans.len() {
        // A phrase over the next few words.
        let mut run = String::new();
        for (n, f) in folded.iter().enumerate().skip(k).take(4) {
            run.push_str(f);
            if n > k && w.joined.contains(&run) {
                whole[k..=n].iter_mut().for_each(|x| *x = true);
            }
        }
        // Letters one at a time.
        let mut n = k;
        let mut spelled = String::new();
        while n < spans.len() && folded[n].chars().count() == 1 {
            spelled.push_str(&folded[n]);
            n += 1;
        }
        if n - k >= 3 && (bad_word(&spelled) || w.joined.contains(&spelled)) {
            whole[k..n].iter_mut().for_each(|x| *x = true);
        }
    }
    let mut out: Vec<char> = chars.clone();
    for (k, &(a, b)) in spans.iter().enumerate() {
        if whole[k] {
            out[a..b].iter_mut().for_each(|c| *c = '*');
        } else if bad[k] {
            out[a + 1..b].iter_mut().for_each(|c| *c = '*');
        }
    }
    out.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn masks_rude_words_only() {
        assert_eq!(mask("Copper Hill"), "Copper Hill");
        assert_eq!(mask("Scunthorpe Sussex"), "Scunthorpe Sussex");
        assert_eq!(mask("Rio Negro Raceway"), "Rio Negro Raceway");
        assert_eq!(mask("Big shit jumps"), "Big s*** jumps");
        assert_eq!(mask("5H1T hill"), "5*** hill");
        assert_eq!(mask("FUCKING fast"), "F****** fast");
        assert_eq!(mask("blow job alley"), "**** *** alley");
        assert!(is_rude("motherfucker"));
        assert!(!is_rude("Assembly Hall"));
        assert!(!is_rude("Class 250"));
        assert!(!is_rude("Therapist Track"));
        assert_eq!(mask("ShitHead"), "S*******");
        assert_eq!(mask("f u c k"), "* * * *");
        assert_eq!(mask("A B C"), "A B C");
    }
}
