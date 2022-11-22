use std::str::FromStr;

use roxmltree::Node;

use crate::mdb::{
    Comparison, ComparisonOperator, Index, IntegerValue, MatchCriteria, MatchCriteriaIdx,
    MissionDatabase, NameReferenceType, ParameterInstanceRef, types::MemberPath,
};

use super::{
    utils::{get_parse_error, read_attribute, read_mandatory_attribute, read_mandatory_text},
    ParseContext, XtceError, XtceParseError,
};

/// parses the match criteria, adds it to the mdb.match_criterias and returns the idx
pub(super) fn read_match_criteria(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<MatchCriteriaIdx, XtceError> {
    for cnode in node.children() {
        let mc = match cnode.tag_name().name() {
            "Comparison" => MatchCriteria::Comparison(read_comparison(mdb, ctx, &cnode)?),
            "ComparisonList" => {
                MatchCriteria::ComparisonList(read_comparison_list(mdb, ctx, &cnode)?)
            }
            "BooleanExpression" => {
                todo!()
            }
            "CustomAlgorithm" => {
                todo!()
            }
            _ => {
                log::warn!("ignoring unknown  '{}'", cnode.tag_name().name());
                continue;
            }
        };

        return Ok(mdb.add_match_criteria(mc));
    }

    Err(XtceError::ParseError(get_parse_error("No criteria specified", node)))
}

pub(super) fn read_comparison(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<Comparison, XtceError> {
    let value = read_mandatory_attribute::<String>(node, "value")?;
    let comparison_operator = (read_attribute::<ComparisonOperator>(node, "comparisonOperator")?)
        .unwrap_or(ComparisonOperator::Equality);
    let param_instance = read_para_insta_ref(mdb, ctx, node)?;

    Ok(Comparison { param_instance, comparison_operator, value })
}

pub(super) fn read_comparison_list(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<Vec<Comparison>, XtceError> {
    let mut r = Vec::new();
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "Comparison" => r.push(read_comparison(mdb, ctx, &cnode)?),
            _ => {
                log::warn!(
                    "ignoring unknown element in comparison list '{}'",
                    cnode.tag_name().name()
                );
                continue;
            }
        }
    }

    Ok(r)
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
    let (pidx, member_path) = resolve_para_ref(mdb, ctx, &pref)?;
    let instance = (read_attribute::<i32>(node, "instance")?).unwrap_or(0);
    let use_calibrated_value =
        (read_attribute::<bool>(node, "useCalibratedValue")?).unwrap_or(true);

    Ok(ParameterInstanceRef { pidx, instance, use_calibrated_value, member_path})
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

pub(super) fn resolve_para_ref(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    name: &str,
) -> Result<(Index, Option<MemberPath>), XtceError> {
    let rtype =  NameReferenceType::Parameter;
    let (qn, rname, aggr_path) = match ctx.name_tree.resolve_ref(name, ctx.path, rtype) {
        Some((qn, ptype_idx, aggr_path)) => (qn, ptype_idx, aggr_path),
        None => {
            return Err(XtceError::UndefinedReference(name.to_string(), rtype));
        }
    };
    let idx = mdb.get_parameter_idx(qn, rname) .ok_or(XtceError::UnresolvedReference(name.to_string(), rtype))?;

    Ok((idx, aggr_path))
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