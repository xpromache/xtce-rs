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

pub(super) fn resolve_ref(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    name: &str,
    rtype: NameReferenceType,
) -> Result<Index, XtceError> {
    let (qn, rname) = match ctx.name_tree.resolve_ref(name, ctx.path, rtype) {
        Some((qn, ptype_idx, _)) => (qn, ptype_idx),
        None => {
            return Err(XtceError::UndefinedReference(name.to_string(), rtype));
        }
    };

    match rtype {
        NameReferenceType::ParameterType => mdb.get_parameter_type_idx(qn, rname),
        NameReferenceType::Parameter => mdb.get_parameter_idx(qn, rname),
        NameReferenceType::SequenceContainer => mdb.get_container_idx(qn, rname),
        NameReferenceType::Algorithm => todo!(),
    }
    .ok_or(XtceError::UnresolvedReference(name.to_string(), rtype))
}

pub(super) fn read_integer_value(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<IntegerValue, XtceParseError> {
    for cnode in node.children() {
        let iv = match cnode.tag_name().name() {
            "FixedValue" => IntegerValue::FixedValue(read_mandatory_text::<i64>(&cnode)?),
            "DynamicValue" => {
                todo!()
            }
            "" => continue,
            _ => {
                return Err(get_parse_error(
                    format!("Invalid elemenent {} for IntegerValue", cnode.tag_name().name()),
                    node,
                ));
            }
        };
        return Ok(iv);
    }

    Err(get_parse_error("Invalid IntegerValue, expected FixedValue or DynamicValue element", node))
}
