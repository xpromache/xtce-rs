use std::str::FromStr;

use roxmltree::Node;

use crate::mdb::{
    types::MemberPath, Comparison, ComparisonOperator, DynamicValueType, Index, IntegerValue,
    LinearAdjustment, MatchCriteria, MatchCriteriaIdx, MissionDatabase, NameReferenceType,
    ParameterInstanceRef,
};

use super::{
    utils::{
        children, get_parse_error, missing, read_attribute, read_mandatory_attribute,
        read_mandatory_text,
    },
    ParseContext, XtceError, XtceParseError, IGNORE_PARAM_NAME, INVALID_PARAM_IDX, Result,
};

/// parses the match criteria, adds it to the mdb.match_criterias and returns the idx
pub(super) fn read_match_criteria(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<MatchCriteriaIdx> {
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
            "" => continue,
            _ => {
                log::warn!(
                    "ignoring unknown element in match criteria '{}'",
                    cnode.tag_name().name()
                );
                continue;
            }
        };

        return Ok(mdb.add_match_criteria(mc));
    }

    Err(get_parse_error("No criteria specified", node))
}

pub(super) fn read_comparison(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<Comparison> {
    let value = read_mandatory_attribute::<String>(node, "value")?;
    let comparison_operator = (read_attribute::<ComparisonOperator>(node, "comparisonOperator")?)
        .unwrap_or(ComparisonOperator::Equality);
    let param_instance = read_para_insta_ref(mdb, ctx, node, false)?;

    Ok(Comparison { param_instance, comparison_operator, value })
}

pub(super) fn read_comparison_list(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<Vec<Comparison>> {
    let mut r = Vec::new();
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "Comparison" => r.push(read_comparison(mdb, ctx, &cnode)?),
            "" => continue,
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
    type Err = XtceError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "==" => Ok(ComparisonOperator::Equality),
            "!=" => Ok(ComparisonOperator::Inequality),
            "<" => Ok(ComparisonOperator::SmallerThan),
            "<=" => Ok(ComparisonOperator::SmallerOrEqualThan),
            ">" => Ok(ComparisonOperator::LargerThan),
            ">=" => Ok(ComparisonOperator::LargerOrEqualThan),
            _ => Err(XtceError::InvalidValue("please use one of == != < <= > >=".to_owned())),
        }
    }
}

/// Read a parameter instance reference
/// if allow_ignore is true, a reference to "__yamcs_ignore" parameter will be accepted and result in a INVALID_PARAM_IDX parameter
/// The caller has to check for that and not use the invalid parameterr
pub(super) fn read_para_insta_ref(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
    allow_ignore: bool
) -> Result<ParameterInstanceRef> {
    let pref = read_mandatory_attribute::<String>(node, "parameterRef")?;
    
    let (pidx, member_path) =  if allow_ignore && pref==IGNORE_PARAM_NAME {
        (INVALID_PARAM_IDX, None)
    } else {
        resolve_para_ref(mdb, ctx, &pref)?
    };

    let instance = (read_attribute::<i32>(node, "instance")?).unwrap_or(0);
    let use_calibrated_value =
        (read_attribute::<bool>(node, "useCalibratedValue")?).unwrap_or(true);

    Ok(ParameterInstanceRef { pidx, instance, use_calibrated_value, member_path })
}

pub(super) fn resolve_ref(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    name: &str,
    rtype: NameReferenceType,
) -> Result<Index> {
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
    .ok_or_else(|| XtceError::UnresolvedReference(name.to_string(), rtype))
}

pub(super) fn resolve_para_ref(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    name: &str,
) -> Result<(Index, Option<MemberPath>)> {
    let rtype = NameReferenceType::Parameter;
    let (qn, rname, aggr_path) = match ctx.name_tree.resolve_ref(name, ctx.path, rtype) {
        Some((qn, ptype_idx, aggr_path)) => (qn, ptype_idx, aggr_path),
        None => {
            return Err(XtceError::UndefinedReference(name.to_string(), rtype));
        }
    };
    let idx = mdb
        .get_parameter_idx(qn, rname)
        .ok_or_else(||XtceError::UnresolvedReference(name.to_string(), rtype))?;

    Ok((idx, aggr_path))
}



pub(super) fn read_integer_value(
    _mdb: &MissionDatabase,
    _ctx: &ParseContext,
    node: &Node,
) -> Result<IntegerValue> {
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

/// Read a dynamic value - that is a value that makes reference to a parameter and has an optional liner adjust factor
/// if allow_ignore is true, a reference to "__yamcs_ignore" parameter will be accepted and result in a INVALID_PARAM_IDX parameter
/// The caller has to check for that and not use that invalid parameterr
pub(super) fn read_dynamic_value(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
    allow_ignore: bool
) -> Result<DynamicValueType> {
    let mut pref = None;
    let mut adjustment = None;
    for cnode in children(node) {
        match cnode.tag_name().name() {
            "ParameterInstanceRef" => {
                pref.replace(read_para_insta_ref(mdb, ctx, &cnode, allow_ignore)?);                
            }
            "LinearAdjustment" => {
                let slope = read_mandatory_attribute::<f64>(&cnode, "slope")?;
                let intercept = read_attribute::<f64>(&cnode, "intercept")?.unwrap_or(0f64);
                adjustment.replace(LinearAdjustment { slope, intercept });
            }
            _ => {
                log::warn!(
                    "ignoring string data encoding dynamic value unknown property '{}'",
                    cnode.tag_name().name()
                );
            }
        }
    }

    if pref.is_none() {
        return Err(missing("element ParameterInstanceRef from", &node));
    }
   

    Ok(DynamicValueType { adjustment, para_ref: pref.unwrap() })
}
