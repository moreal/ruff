use rustpython_parser::ast::Location;

use crate::core::locator::Locator;
use crate::core::types::Range;

/// Return the leading quote for a string or byte literal (e.g., `"""`).
pub fn leading_quote(content: &str) -> Option<&str> {
    if let Some(first_line) = content.lines().next() {
        for pattern in ruff_python::str::TRIPLE_QUOTE_PREFIXES
            .iter()
            .chain(ruff_python::bytes::TRIPLE_QUOTE_PREFIXES)
            .chain(ruff_python::str::SINGLE_QUOTE_PREFIXES)
            .chain(ruff_python::bytes::SINGLE_QUOTE_PREFIXES)
        {
            if first_line.starts_with(pattern) {
                return Some(pattern);
            }
        }
    }
    None
}

/// Return the trailing quote string for a string or byte literal (e.g., `"""`).
pub fn trailing_quote(content: &str) -> Option<&&str> {
    ruff_python::str::TRIPLE_QUOTE_SUFFIXES
        .iter()
        .chain(ruff_python::str::SINGLE_QUOTE_SUFFIXES)
        .find(|&pattern| content.ends_with(pattern))
}

pub fn is_radix_literal(content: &str) -> bool {
    content.starts_with("0b")
        || content.starts_with("0o")
        || content.starts_with("0x")
        || content.starts_with("0B")
        || content.starts_with("0O")
        || content.starts_with("0X")
}

/// Expand the range of a compound statement.
///
/// `location` is the start of the compound statement (e.g., the `if` in `if x:`).
/// `end_location` is the end of the last statement in the body.
pub fn expand_indented_block(
    location: Location,
    end_location: Location,
    locator: &Locator,
) -> (Location, Location) {
    let contents = locator.contents();
    let start_index = locator.index(location);
    let end_index = locator.index(end_location);

    // Find the colon, which indicates the end of the header.
    let mut nesting = 0;
    let mut colon = None;
    for (start, tok, _end) in rustpython_parser::lexer::lex_located(
        &contents[start_index..end_index],
        rustpython_parser::Mode::Module,
        location,
    )
    .flatten()
    {
        match tok {
            rustpython_parser::Tok::Colon if nesting == 0 => {
                colon = Some(start);
                break;
            }
            rustpython_parser::Tok::Lpar
            | rustpython_parser::Tok::Lsqb
            | rustpython_parser::Tok::Lbrace => nesting += 1,
            rustpython_parser::Tok::Rpar
            | rustpython_parser::Tok::Rsqb
            | rustpython_parser::Tok::Rbrace => nesting -= 1,
            _ => {}
        }
    }
    let colon_location = colon.unwrap();
    let colon_index = locator.index(colon_location);

    // From here, we have two options: simple statement or compound statement.
    let indent = rustpython_parser::lexer::lex_located(
        &contents[colon_index..end_index],
        rustpython_parser::Mode::Module,
        colon_location,
    )
    .flatten()
    .find_map(|(start, tok, _end)| match tok {
        rustpython_parser::Tok::Indent => Some(start),
        _ => None,
    });

    let Some(indent_location) = indent else {
        // Simple statement: from the colon to the end of the line.
        return (colon_location, Location::new(end_location.row() + 1, 0));
    };

    // Compound statement: from the colon to the end of the block.
    let mut offset = 0;
    for (index, line) in contents[end_index..].lines().skip(1).enumerate() {
        if line.is_empty() {
            continue;
        }

        if line
            .chars()
            .take(indent_location.column())
            .all(char::is_whitespace)
        {
            offset = index + 1;
        } else {
            break;
        }
    }

    let end_location = Location::new(end_location.row() + 1 + offset, 0);
    (colon_location, end_location)
}

/// Return true if the `orelse` block of an `if` statement is an `elif` statement.
pub fn is_elif(orelse: &[rustpython_parser::ast::Stmt], locator: &Locator) -> bool {
    if orelse.len() == 1 && matches!(orelse[0].node, rustpython_parser::ast::StmtKind::If { .. }) {
        let (source, start, end) = locator.slice(Range::new(
            orelse[0].location,
            orelse[0].end_location.unwrap(),
        ));
        if source[start..end].starts_with("elif") {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    #[test]
    fn test_prefixes() {
        let prefixes = ruff_python::str::TRIPLE_QUOTE_PREFIXES
            .iter()
            .chain(ruff_python::bytes::TRIPLE_QUOTE_PREFIXES)
            .chain(ruff_python::str::SINGLE_QUOTE_PREFIXES)
            .chain(ruff_python::bytes::SINGLE_QUOTE_PREFIXES)
            .collect::<Vec<_>>();
        for i in 1..prefixes.len() {
            for j in 0..i - 1 {
                if i != j {
                    if prefixes[i].starts_with(prefixes[j]) {
                        assert!(
                            !prefixes[i].starts_with(prefixes[j]),
                            "Prefixes are not unique: {} starts with {}",
                            prefixes[i],
                            prefixes[j]
                        );
                    }
                }
            }
        }
    }
}
