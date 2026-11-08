//! Headers and footers: text printed in the top and bottom margins of every page.
//!
//! Each of the two bands has a left, centre and right piece, and the first page
//! may have its own. The text may hold fields that are filled in per page:
//! `{page}`, `{pages}` and `{title}`. There is no date field on purpose: an export
//! made tomorrow should be the same as one made today.

use serde::{Deserialize, Serialize};

/// The longest piece of header or footer text, in characters.
pub const MAX_HEADER_CHARS: usize = 120;

/// One band: three pieces of text.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PageText {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub left: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub centre: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub right: String,
}

impl PageText {
    pub fn is_blank(&self) -> bool {
        self.left.is_empty() && self.centre.is_empty() && self.right.is_empty()
    }

    /// The three pieces with their fields filled in, left to right.
    pub fn expanded(&self, page: usize, pages: usize, title: &str) -> [String; 3] {
        [&self.left, &self.centre, &self.right].map(|t| expand(t, page, pages, title))
    }

    fn cleaned(&self) -> PageText {
        let clean = |s: &String| -> String {
            s.chars()
                .filter(|c| !c.is_control())
                .take(MAX_HEADER_CHARS)
                .collect()
        };
        PageText {
            left: clean(&self.left),
            centre: clean(&self.centre),
            right: clean(&self.right),
        }
    }
}

/// Fills in `{page}`, `{pages}` and `{title}`. Anything else in braces stays as
/// typed. `page` counts from 1.
pub fn expand(text: &str, page: usize, pages: usize, title: &str) -> String {
    text.replace("{page}", &page.to_string())
        .replace("{pages}", &pages.to_string())
        .replace("{title}", title)
}

/// All the header and footer text of a worksheet.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeaderFooter {
    #[serde(default, skip_serializing_if = "PageText::is_blank")]
    pub header: PageText,
    #[serde(default, skip_serializing_if = "PageText::is_blank")]
    pub footer: PageText,
    /// Use `first_header` and `first_footer` on the first page.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub different_first: bool,
    #[serde(default, skip_serializing_if = "PageText::is_blank")]
    pub first_header: PageText,
    #[serde(default, skip_serializing_if = "PageText::is_blank")]
    pub first_footer: PageText,
}

impl HeaderFooter {
    pub fn is_default(&self) -> bool {
        *self == HeaderFooter::default()
    }

    /// The header and footer to print on page `k` (counting from 0).
    pub fn for_page(&self, k: usize) -> (&PageText, &PageText) {
        if self.different_first && k == 0 {
            (&self.first_header, &self.first_footer)
        } else {
            (&self.header, &self.footer)
        }
    }

    /// The same with text cut to length and control characters removed.
    pub fn sanitised(&self) -> HeaderFooter {
        HeaderFooter {
            header: self.header.cleaned(),
            footer: self.footer.cleaned(),
            different_first: self.different_first,
            first_header: self.first_header.cleaned(),
            first_footer: self.first_footer.cleaned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fields_are_filled_in_and_other_braces_stay() {
        assert_eq!(
            expand("Page {page} of {pages} - {title} {x} {", 2, 5, "Beam"),
            "Page 2 of 5 - Beam {x} {"
        );
        assert_eq!(expand("", 1, 1, "t"), "");
    }

    #[test]
    fn the_first_page_can_have_its_own() {
        let h = HeaderFooter {
            header: PageText {
                centre: "all".into(),
                ..PageText::default()
            },
            first_header: PageText {
                centre: "first".into(),
                ..PageText::default()
            },
            different_first: true,
            ..HeaderFooter::default()
        };
        assert_eq!(h.for_page(0).0.centre, "first");
        assert_eq!(h.for_page(1).0.centre, "all");
        let plain = HeaderFooter {
            different_first: false,
            ..h
        };
        assert_eq!(plain.for_page(0).0.centre, "all");
    }

    #[test]
    fn nothing_is_saved_for_a_blank_setup_and_text_round_trips() {
        assert_eq!(
            serde_json::to_string(&HeaderFooter::default()).unwrap(),
            "{}"
        );
        let h = HeaderFooter {
            footer: PageText {
                right: "{page}/{pages}".into(),
                ..PageText::default()
            },
            ..HeaderFooter::default()
        };
        let back: HeaderFooter = serde_json::from_str(&serde_json::to_string(&h).unwrap()).unwrap();
        assert_eq!(back, h);
    }

    #[test]
    fn sanitising_cuts_and_cleans() {
        let h = HeaderFooter {
            header: PageText {
                left: format!("a\nb{}", "x".repeat(500)),
                ..PageText::default()
            },
            ..HeaderFooter::default()
        };
        let s = h.sanitised();
        assert_eq!(s.header.left.chars().count(), MAX_HEADER_CHARS);
        assert!(!s.header.left.contains('\n'));
    }
}
