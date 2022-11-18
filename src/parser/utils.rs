use super::*;

use roxmltree::Node;

pub(super) fn get_parse_error<S: AsRef<str>>(msg: S, node: &Node) -> XtceParseError {
    let pos = node.document().text_pos_at(node.range().start);
    XtceParseError { msg: msg.as_ref().to_owned(), pos }
}

pub(super) fn read_mandatory_text<T: std::str::FromStr>(node: &Node) -> Result<T, XtceParseError> {
    let x = read_text::<T>(node)?;
    match x {
        None => Err(get_parse_error(format!("Cannot find text"), node)),
        Some(v) => Ok(v),
    }
}

pub(super) fn read_text<T: std::str::FromStr>(node: &Node) -> Result<Option<T>, XtceParseError> {
    if let Some(strv) = node.text() {
        match strv.parse::<T>() {
            Ok(n) => Ok(Some(n)),
            Err(_) => Err(get_parse_error(format!("Cannot parse value '{}'", strv), node)),
        }
    } else {
        Ok(Option::None)
    }
}

pub(super) fn read_mandatory_attribute<T: std::str::FromStr>(
    node: &Node,
    attr_name: &str,
) -> Result<T, XtceParseError> {
    let x = read_attribute::<T>(node, attr_name)?;
    match x {
        None => Err(get_parse_error(format!("Cannot find attribute {}", attr_name), node)),
        Some(v) => Ok(v),
    }
}

pub(super) fn read_name_description(ctx: &ParseContext) -> NameDescription {
    let node = &ctx.node;
    let mut nd = NameDescription::new(ctx.name);
    nd.short_description = node.attribute("shortDescription").map(|s| s.to_string());

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "LongDescription" => nd.long_description = node.text().map(|s| s.to_string()),
            _ => {}
        }
    }
    nd
}

pub(super) fn read_mandatory_name<'a>(node: &'a Node) -> Result<&'a str, XtceParseError> {
    node.attribute("name")
        .ok_or_else(|| get_parse_error("Cannot find mandatory attribute name", node))
}

pub(super) fn read_attribute<T: std::str::FromStr>(
    node: &Node,
    attr_name: &str,
) -> Result<Option<T>, XtceParseError> {
    if let Some(strv) = node.attribute(attr_name) {
        match strv.parse::<T>() {
            Ok(n) => Ok(Some(n)),
            Err(_) => Err(get_parse_error(
                format!("Cannot parse value '{}' for attribute {}", strv, attr_name),
                node,
            )),
        }
    } else {
        Ok(Option::None)
    }
}
