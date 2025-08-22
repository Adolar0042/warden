use std::fmt;

use dialoguer::console::{Style, StyledObject, style};
use dialoguer::theme::Theme;
use fuzzy_matcher::FuzzyMatcher as _;
use fuzzy_matcher::skim::SkimMatcherV2;

#[derive(Debug, Clone)]
pub struct InputTheme {
    pub prompt_suffix: StyledObject<String>,
    pub selected_suffix: StyledObject<String>,
    pub active_prefix: StyledObject<String>,
    pub inactive_prefix: StyledObject<String>,
    pub checked: StyledObject<String>,
    pub unchecked: StyledObject<String>,
    pub error_prefix: StyledObject<String>,
    pub item_style: Style,
    pub active_item_style: Style,
    pub result_style: Style,
}

impl Default for InputTheme {
    fn default() -> Self {
        Self {
            prompt_suffix: style(": ".to_string()).bold().for_stderr(),
            selected_suffix: style(" ".to_string()).for_stderr(),
            active_prefix: style("> ".to_string()).bold().magenta().for_stderr(),
            inactive_prefix: style("  ".to_string()).for_stderr(),
            checked: style("[x] ".to_string()).bold().for_stderr(),
            unchecked: style("[ ] ".to_string()).for_stderr(),
            error_prefix: style("error:".to_string()).red().bold().for_stderr(),
            item_style: Style::new().for_stderr(),
            active_item_style: Style::new().blue().for_stderr(),
            result_style: Style::new().green().for_stderr(),
        }
    }
}

impl Theme for InputTheme {
    fn format_prompt(&self, f: &mut dyn fmt::Write, prompt: &str) -> fmt::Result {
        if prompt.is_empty() {
            Ok(())
        } else {
            write!(f, "{}{} ", prompt, self.prompt_suffix)
        }
    }

    fn format_error(&self, f: &mut dyn fmt::Write, err: &str) -> fmt::Result {
        write!(f, "{} {}", self.error_prefix, err)
    }

    fn format_input_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        default: Option<&str>,
    ) -> fmt::Result {
        match default {
            Some(d) if prompt.is_empty() => {
                write!(
                    f,
                    "{}{}",
                    style(format!("({d})")).dim().for_stderr(),
                    self.prompt_suffix
                )
            },
            Some(d) => {
                write!(
                    f,
                    "{} {}{}",
                    style(prompt).bold().for_stderr(),
                    style(format!("({d})")).dim().for_stderr(),
                    self.prompt_suffix
                )
            },
            None => {
                write!(
                    f,
                    "{}{}",
                    style(prompt).bold().for_stderr(),
                    self.prompt_suffix
                )
            },
        }
    }

    fn format_input_prompt_selection(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        sel: &str,
    ) -> std::fmt::Result {
        write!(
            f,
            "{}{}{}",
            style(prompt).bold().for_stderr(),
            self.selected_suffix,
            &self.result_style.apply_to(sel)
        )
    }

    fn format_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
    ) -> fmt::Result {
        write!(
            f,
            "{}{}",
            if active {
                &self.active_prefix
            } else {
                &self.inactive_prefix
            },
            if active {
                style(text).blue().for_stderr()
            } else {
                style(text).for_stderr()
            }
        )
    }

    fn format_multi_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        checked: bool,
        active: bool,
    ) -> fmt::Result {
        write!(
            f,
            "{}{}{text}",
            if active {
                &self.active_prefix
            } else {
                &self.inactive_prefix
            },
            if checked {
                &self.checked
            } else {
                &self.unchecked
            }
        )
    }

    fn format_sort_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        picked: bool,
        active: bool,
    ) -> fmt::Result {
        write!(
            f,
            "{}{}{text}",
            if active {
                &self.active_prefix
            } else {
                &self.inactive_prefix
            },
            if picked {
                &self.checked
            } else {
                &self.unchecked
            }
        )
    }

    fn format_confirm_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        default: Option<bool>,
    ) -> fmt::Result {
        if !prompt.is_empty() {
            write!(f, "{} ", style(prompt).bold().for_stderr())?;
        }
        match default {
            None => write!(f, "{}", style("[y/n] ").bold().for_stderr()),
            Some(true) => write!(f, "{}", style("[Y/n] ").bold().for_stderr()),
            Some(false) => write!(f, "{}", style("[y/N] ").bold().for_stderr()),
        }
    }

    fn format_confirm_prompt_selection(
        &self,
        f: &mut dyn std::fmt::Write,
        prompt: &str,
        selection: Option<bool>,
    ) -> std::fmt::Result {
        let chosen = match selection {
            Some(true) => "yes",
            Some(false) => "no",
            None => "",
        };

        write!(
            f,
            "{}{}{}",
            style(prompt).bold().for_stderr(),
            &self.selected_suffix,
            &self.result_style.apply_to(chosen)
        )
    }

    fn format_fuzzy_select_prompt_item(
        &self,
        f: &mut dyn fmt::Write,
        text: &str,
        active: bool,
        highlight_matches: bool,
        matcher: &SkimMatcherV2,
        search_term: &str,
    ) -> fmt::Result {
        write!(
            f,
            "{}",
            if active {
                &self.active_prefix
            } else {
                &self.inactive_prefix
            }
        )?;

        let base = if active {
            self.active_item_style.clone()
        } else {
            self.item_style.clone()
        };
        let base_bold = base.clone().bold();

        if highlight_matches
            && let Some((_score, indices)) = matcher.fuzzy_indices(text, search_term)
        {
            // indices are byte offsets; iterate with char_indices
            let mut j = 0;
            for (byte_idx, ch) in text.char_indices() {
                if j < indices.len() && indices[j] == byte_idx {
                    write!(f, "{}", base_bold.apply_to(ch))?;
                    j += 1;
                } else {
                    write!(f, "{}", base.apply_to(ch))?;
                }
            }
            return Ok(());
        }

        write!(f, "{}", base.apply_to(text))
    }

    fn format_fuzzy_select_prompt(
        &self,
        f: &mut dyn fmt::Write,
        prompt: &str,
        search_term: &str,
        bytes_pos: usize,
    ) -> fmt::Result {
        if !prompt.is_empty() {
            write!(
                f,
                "{}{}",
                style(prompt).bold().for_stderr(),
                self.prompt_suffix
            )?;
        }

        // clamp and align to a char boundary to avoid UTF-8 panics
        let mut pos = bytes_pos.min(search_term.len());
        while !search_term.is_char_boundary(pos) {
            pos -= 1;
        }

        let (head, tail) = search_term.split_at(pos);
        write!(f, "{head}|{tail}")
    }
}
