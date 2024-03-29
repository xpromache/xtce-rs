use roxmltree::Node;

use super::{misc::resolve_ref, *};

use encodings::*;

use crate::mdb::{
    types::{
        AbsoluteTimeDataType, AggregateDataType, ArrayDataType, BinaryDataType, BooleanDataType,
        DataEncoding, DataType, EnumeratedDataType, FloatDataType, IntegerDataType, Member,
        StringDataType, TypeData, ValueEnumeration,
    },
    *,
};

pub(super) fn add_parameter_type(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
) -> Result<()> {
    let (encoding, type_data) = match ctx.node.tag_name().name() {
        "IntegerParameterType" => read_integer_parameter_type(mdb, ctx)?,
        "FloatParameterType" => read_float_parameter_type(mdb, ctx)?,
        "EnumeratedParameterType" => read_enumerated_parameter_type(mdb, ctx)?,
        "BooleanParameterType" => read_boolean_parameter_type(mdb, ctx)?,
        "StringParameterType" => read_string_parameter_type(mdb, ctx)?,
        "BinaryParameterType" => read_binary_parameter_type(mdb, ctx)?,
        "AbsoluteTimeParameterType" => read_absolute_time_parameter_type(mdb, ctx)?,
        "AggregateParameterType" => read_aggregate_parameter_type(mdb, ctx)?,
        "ArrayParameterType" => read_array_parameter_type(mdb, ctx)?,
        _ => {
            log::warn!("ignoring parameter type '{}'", ctx.node.tag_name().name());
            return Ok(());
        }
    };
    let dtype = DataType {
        ndescr: read_name_description(ctx),
        encoding,
        units: read_unit_set(&ctx.node)?,
        type_data,
        calibrator: None,
    };

    mdb.add_parameter_type(ctx.path, dtype);
    Ok(())
}

pub(super) fn read_integer_parameter_type(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
) -> Result<(DataEncoding, TypeData)> {
    let mut encoding = DataEncoding::None;
    let signed = read_attribute::<bool>(&ctx.node, "signed")?.unwrap_or(true);
    let size_in_bits = read_attribute::<u32>(&ctx.node, "sizeInBits")?.unwrap_or(32);

    for cnode in ctx.node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(
                    mdb,
                    ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "" | "LongDescription" | "UnitSet" => {}
            _ => log::warn!(
                "ignoring integer parameter type  unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }

    let ipt = IntegerDataType { size_in_bits, signed, default_alarm: None, context_alarm: vec![] };

    Ok((encoding, TypeData::Integer(ipt)))
}

pub(super) fn read_float_parameter_type(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
) -> Result<(DataEncoding, TypeData)> {
    let mut encoding = DataEncoding::None;

    for cnode in ctx.node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(
                    mdb,
                    &ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "FloatDataEncoding" => {
                encoding = DataEncoding::Float(read_float_data_encoding(
                    mdb,
                    &ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "" | "LongDescription" | "UnitSet" => {}
            _ => log::warn!(
                "ignoring float parameter type unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }

    let fpt = FloatDataType { size_in_bits: 0, default_alarm: None, context_alarm: vec![] };

    Ok((encoding, TypeData::Float(fpt)))
}

pub(super) fn read_boolean_parameter_type(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
) -> Result<(DataEncoding, TypeData)> {
    let node = &ctx.node;
    let osv = read_attribute::<String>(node, "oneStringValue")?.unwrap_or("True".to_owned());
    let zsv = read_attribute::<String>(node, "zeroStringValue")?.unwrap_or("False".to_owned());

    let mut encoding = DataEncoding::None;

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(
                    mdb,
                    &ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "FloatDataEncoding" => {
                encoding = DataEncoding::Float(read_float_data_encoding(
                    mdb,
                    &ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "" | "LongDescription" | "UnitSet" => {}
            _ => log::warn!(
                "ignoring boolean parameter type unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }

    let bpt = BooleanDataType { one_string_value: osv, zero_string_value: zsv };

    Ok((encoding, TypeData::Boolean(bpt)))
}

pub(super) fn read_enumerated_parameter_type(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
) -> Result<(DataEncoding, TypeData)> {
    let mut encoding = DataEncoding::None;
    let mut enumeration = Vec::<ValueEnumeration>::new();

    for cnode in ctx.node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(
                    mdb,
                    &ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "FloatDataEncoding" => {
                encoding = DataEncoding::Float(read_float_data_encoding(
                    mdb,
                    &ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "EnumerationList" => {
                read_enumeration_list(&mut enumeration, &cnode)?;
            }
            "" | "LongDescription" | "UnitSet" => {}
            _ => log::warn!(
                "ignoring enumerated parameter type unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }

    let ept = EnumeratedDataType { enumeration, default_alarm: None, context_alarm: vec![] };
    Ok((encoding, TypeData::Enumerated(ept)))
}

pub(super) fn read_string_parameter_type(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
) -> Result<(DataEncoding, TypeData)> {
    let mut encoding = DataEncoding::None;

    for cnode in ctx.node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(
                    mdb,
                    &ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "FloatDataEncoding" => {
                encoding = DataEncoding::Float(read_float_data_encoding(
                    mdb,
                    &ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "StringDataEncoding" => {
                encoding = DataEncoding::String(read_string_data_encoding(
                    mdb,
                    &ctx,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "" | "LongDescription" | "UnitSet" => {}
            _ => log::warn!(
                "ignoring string parameter type unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }

    let spt = StringDataType {};

    Ok((encoding, TypeData::String(spt)))
}

pub(super) fn read_binary_parameter_type(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
) -> Result<(DataEncoding, TypeData)> {
    let mut encoding = DataEncoding::None;

    for cnode in ctx.node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(
                    mdb,
                    &ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "FloatDataEncoding" => {
                encoding = DataEncoding::Float(read_float_data_encoding(
                    mdb,
                    &ctx.path,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "StringDataEncoding" => {
                encoding = DataEncoding::String(read_string_data_encoding(
                    mdb,
                    &ctx,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "BinaryDataEncoding" => {
                encoding = DataEncoding::Binary(read_binary_data_encoding(
                    mdb,
                    &ctx,
                    &cnode,
                    &DataEncoding::None,
                )?);
            }
            "" | "LongDescription" | "UnitSet" => {}
            _ => log::warn!(
                "ignoring binary parameter type unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }

    let mut bpt = BinaryDataType { size_in_bits: 32 };

    Ok((encoding, TypeData::Binary(bpt)))
}

//reads an aggregate parameter type from the XTCE
pub(super) fn read_aggregate_parameter_type(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
) -> Result<(DataEncoding, TypeData)> {
    let mut members = Vec::new();

    for cnode in ctx.node.children() {
        match cnode.tag_name().name() {
            "MemberList" => {
                for mnode in cnode.children() {
                    match mnode.tag_name().name() {
                        "Member" => members.push(read_member(mdb, ctx, &mnode)?),
                        "" => continue,
                        _ => log::warn!(
                            "ignoring member list unknown property '{}'",
                            mnode.tag_name().name()
                        ),
                    }
                }
            }
            "" | "LongDescription" => {}
            _ => log::warn!(
                "ignoring aggreagate parameter type unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }

    let apt = AggregateDataType { members };
    Ok((DataEncoding::None, TypeData::Aggregate(apt)))
}

// reads a member of an aggregate type from the XTCE
fn read_member(mdb: &mut MissionDatabase, ctx: &ParseContext, node: &Node) -> Result<Member> {
    let ptype_str = read_mandatory_attribute::<String>(node, "typeRef")?;
    let rtype = NameReferenceType::ParameterType;

    let dtype = resolve_ref(mdb, ctx, &ptype_str, rtype)?;
    let name_str = read_mandatory_name(node)?;
    let name = mdb.get_or_intern(name_str);
    let ctx1 = ParseContext {
        name_tree: ctx.name_tree,
        node: *node,
        path: ctx.path,
        name: name,
        rtype: ctx.rtype,
    };
    let ndescr = read_name_description(&ctx1);

    Ok(Member { ndescr, dtype })
}

pub(super) fn read_array_parameter_type(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
) -> Result<(DataEncoding, TypeData)> {
    let ptype_str = read_mandatory_attribute::<String>(&ctx.node, "arrayTypeRef")?;
    let rtype = NameReferenceType::ParameterType;
    let dtype = resolve_ref(mdb, ctx, &ptype_str, rtype)?;

    let apt = ArrayDataType { dim: Vec::new(), dtype };

    Ok((DataEncoding::None, TypeData::Array(apt)))
}

pub(super) fn read_absolute_time_parameter_type(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
) -> Result<(DataEncoding, TypeData)> {
    for cnode in ctx.node.children() {
        match cnode.tag_name().name() {
            "Encoding" => {
                //TODO
            }
            "ReferenceTime" => {
                //TODO
            }
            "" => {}
            _ => {
                log::warn!(
                    "ignoring read_absolute_time_parameter_type '{}'",
                    cnode.tag_name().name()
                )
            }
        };
    }
    let apt = AbsoluteTimeDataType {};
    Ok((DataEncoding::None, TypeData::AbsoluteTime(apt)))
}

fn read_enumeration_list(elist: &mut Vec<ValueEnumeration>, node: &Node) -> Result<()> {
    for cnode in node.children().filter(|n| !n.tag_name().name().is_empty()) {
        let value = read_mandatory_attribute::<i64>(&cnode, "value")?;
        let label = read_mandatory_attribute::<String>(&cnode, "label")?;
        let max_value = read_attribute::<i64>(&cnode, "value")?.unwrap_or(value);
        let description = read_attribute::<String>(&cnode, "shortDescription")?;

        elist.push(ValueEnumeration { value, label, max_value, description });
    }
    Ok(())
}

fn read_unit_set(node: &Node) -> Result<Vec<UnitType>> {
    let mut units = Vec::new();
    for pnode in node.children() {
        match pnode.tag_name().name() {
            "UnitSet" => {
                for cnode in node.children() {
                    match cnode.tag_name().name() {
                        "Unit" => {
                            let power = read_attribute::<f64>(&cnode, "power")?.unwrap_or(1f64);
                            let factor = read_attribute::<String>(&cnode, "factor")?
                                .unwrap_or("1".to_owned());
                            let description = read_attribute::<String>(&cnode, "shortDescription")?;
                            let unit = cnode.text().ok_or_else(|| {
                                get_parse_error("No unit present".to_owned(), &cnode)
                            })?;

                            units.push(UnitType {
                                unit: unit.to_owned(),
                                power,
                                factor,
                                description,
                            });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    Ok(units)
}
