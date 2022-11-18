use std::str::FromStr;

use roxmltree::Node;

use crate::{mdb::{Comparison, ComparisonOperator, MatchCriteria, MissionDatabase, ParameterInstanceRef, NameReferenceType, Index, IntegerValue}};

use super::{
    utils::{get_parse_error, read_attribute, read_mandatory_attribute, read_mandatory_text},
    ParseContext, XtceError, XtceParseError,
};

pub(super) fn read_match_criteria(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<MatchCriteria, XtceError> {
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "Comparison" => {
                return Ok(MatchCriteria::Comparison(read_comparison(mdb, ctx, &cnode)?))
            }
            "ComparisonList" => {
                todo!()
            }
            "BooleanExpression" => {
                todo!()
            }
            "CustomAlgorithm" => {
                todo!()
            }
            _ => log::warn!("ignoring unknown  '{}'", cnode.tag_name().name()),
        }
    }

    Err(XtceError::ParseError(get_parse_error("No criteria specified", node)))
}

pub(super) fn read_comparison(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<Comparison, XtceError> {
    let value = read_mandatory_attribute::<String>(node, "value")?;
    let op = (read_attribute::<ComparisonOperator>(node, "comparisonOperator")?)
        .unwrap_or(ComparisonOperator::Equality);
    let pref = read_para_insta_ref(mdb, ctx, node)?;
    let para = mdb.get_parameter(pref.pidx);
    //let v = para.p

    print!(" value: {} op: {:?} pref: {:?}", value, op, pref);

    todo!()
}

impl FromStr for ComparisonOperator {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "==" => Ok(ComparisonOperator::Equality),
            "!=" => Ok(ComparisonOperator::Inequality),
            "<" => Ok(ComparisonOperator::SmallerThan),
            "<=" => Ok(ComparisonOperator::SmallerOrEqualThan),
            ">" => Ok(ComparisonOperator::LargerThan),
            ">=" => Ok(ComparisonOperator::LargerOrEqualThan),
            _ => Err("please use one of == != < <= > >=".to_owned()),
        }
    }
}

pub(super) fn read_para_insta_ref(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<ParameterInstanceRef, XtceError> {
    let pref = read_mandatory_attribute::<String>(node, "parameterRef")?;
    let pidx = resolve_ref(mdb, ctx, &pref, NameReferenceType::Parameter)?;
    let instance = (read_attribute::<i32>(node, "instance")?).unwrap_or(0);
    let use_calibrated_value = (read_attribute::<bool>(node, "useCalibratedValue")?).unwrap_or(true);

    Ok(ParameterInstanceRef { pidx, instance, use_calibrated_value})
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
