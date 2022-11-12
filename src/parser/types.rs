use roxmltree::{Node};

use super::*;
use utils::*;

use encodings::*;

use crate::mdb::*;

pub(super) fn add_parameter_type(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<(), XtceError> {

    let ptype = match ctx.node.tag_name().name() {
        "IntegerParameterType" => read_integer_parameter_type(mdb, ctx)?,
        /*"FloatParameterType" => read_float_parameter_type(mdb, path, &node),
        "EnumeratedParameterType" => read_enumerated_parameter_type(mdb, path, &node),
        "BooleanParameterType" => read_boolean_parameter_type(mdb, path, &node),
        "StringParameterType" => read_string_parameter_type(mdb, path, &node),
        "AbsoluteTimeParameterType" => read_absolute_time_parameter_type(mdb, path, &node)?,
        "AggregateParameterType" =>  read_aggregate_parameter_type(mdb, path, &node)?,*/
        _ => {
            println!("ignoring read_parameter_type '{}'", ctx.node.tag_name().name());
           return Ok(())
        }
    };
    mdb.add_parameter_type(ctx.path, ptype);
    Ok(())
}


pub(super) fn read_integer_parameter_type(mdb: &MissionDatabase, ctx: &ParseContext) -> Result<ParameterType, XtceError> {
    let name = ctx.name;
    let mut encoding = DataEncoding::None;
    let mut units = vec![];
    let signed = read_attribute::<bool>(&ctx.node, "signed")?.unwrap_or(true);

    for cnode in ctx.node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(mdb, ctx.path, &cnode, &DataEncoding::None)?);
            }
            "UnitSet" => read_unit_set(&mut units, &cnode)?,
            "" | "LongDescription" => {}
            _ => log::warn!("ignoring integer parameter type  unknown property '{}'", cnode.tag_name().name())
        };
    }

    let mut ipt = IntegerParameterType {
        name: NameDescription::new(name),
        size_in_bits: 0,
        signed,
        encoding,
        default_alarm: None,
        context_alarm: vec![],
        units,
    };
    read_name_description(&mut ipt.name, &ctx.node);


    Ok(ParameterType::Integer(ipt))
}
/*

pub(super) fn read_float_parameter_type(mdb: &mut MissionDatabase, xnn: &mut XtceNamedNode, node: &Node) -> Result<ParameterType, XtceError> {
    let name = xnn.name;
    let mut encoding = DataEncoding::None;
    let mut units = vec![];

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(mdb, &xnn.path, &cnode, &DataEncoding::None)?);
            }
            "FloatDataEncoding" => {
                encoding = DataEncoding::Float(read_float_data_encoding(mdb, &xnn.path, &cnode, &DataEncoding::None)?);
            }
            "UnitSet" => read_unit_set(&mut units, &cnode)?,
            "" | "LongDescription" => {}
            _ => println!("ignoring read_float_parameter_type '{}'", cnode.tag_name().name())
        };
    }

    let mut fpt = FloatParameterType {
        name: NameDescription::new(name),
        size_in_bits: 0,
        encoding,
        default_alarm: None,
        context_alarm: vec![],
        units,
    };
    read_name_description(&mut fpt.name, node);

    Ok(ParameterType::Float(fpt))
}


pub(super) fn read_boolean_parameter_type(mdb: &mut MissionDatabase, path: &QualifiedName, node: &Node) -> Result<ParameterType, XtceError> {
    let name = xnn.name;
    let mut encoding = DataEncoding::None;
    let osv = read_attribute::<String>(node, "oneStringValue")?.unwrap_or("True".to_owned());
    let zsv = read_attribute::<String>(node, "zeroStringValue")?.unwrap_or("False".to_owned());
    let mut units = vec![];

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(mdb, &xnn.path, &cnode, &DataEncoding::None)?);
            }
            "FloatDataEncoding" => {
                encoding = DataEncoding::Float(read_float_data_encoding(mdb, &xnn.path, &cnode, &DataEncoding::None)?);
            }
            "UnitSet" => read_unit_set(&mut units, &cnode)?,
            "" | "LongDescription" => {}
            _ => println!("ignoring read_enumerated_parameter_type '{}'", cnode.tag_name().name())
        };
    }

    let mut bpt = BooleanParameterType {
        name: NameDescription::new(name),
        encoding,
        one_string_value: osv,
        zero_string_value: zsv,
        units,
    };
    read_name_description(&mut bpt.name, node);


    Ok(ParameterType::Boolean(bpt))
}

pub(super) fn read_enumerated_parameter_type(mdb: &mut MissionDatabase, path: &QualifiedName, node: &Node) -> Result<ParameterType, XtceError> {
    let name = xnn.name;
    let mut encoding = DataEncoding::None;
    let mut enumeration = Vec::<EnumeratedValue>::new();
    let mut units = vec![];

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(mdb, &xnn.path, &cnode, &DataEncoding::None)?);
            }
            "FloatDataEncoding" => {
                encoding = DataEncoding::Float(read_float_data_encoding(mdb, &xnn.path, &cnode, &DataEncoding::None)?);
            }
            "EnumerationList" => {
                read_enumeration_list(&mut enumeration, &cnode)?;
            }
            "UnitSet" => read_unit_set(&mut units, &cnode)?,
            "" | "LongDescription" => {}
            _ => println!("ignoring read_enumerated_parameter_type '{}'", cnode.tag_name().name())
        };
    }

    let mut ept = EnumeratedParameterType {
        name: NameDescription::new(name),
        encoding,
        enumeration,
        default_alarm: None,
        context_alarm: vec![],
        units,
    };
    read_name_description(&mut ept.name, node);

    Ok(ParameterType::Enumerated(ept))
}


pub(super) fn read_string_parameter_type(mdb: &mut MissionDatabase, path: &QualifiedName, node: &Node) -> Result<ParameterType, XtceError> {
    let name = xnn.name;
    let mut encoding = DataEncoding::None;
    let mut units = vec![];

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" => {
                encoding = DataEncoding::Integer(read_integer_data_encoding(mdb, &xnn.path, &cnode, &DataEncoding::None)?);
            }
            "FloatDataEncoding" => {
                encoding = DataEncoding::Float(read_float_data_encoding(mdb, &xnn.path, &cnode, &DataEncoding::None)?);
            }
            "StringDataEncoding" => {
                encoding = DataEncoding::String(read_string_data_encoding(mdb, &xnn.path, &cnode, &DataEncoding::None)?);
            }
            "UnitSet" => read_unit_set(&mut units, &cnode)?,
            "" | "LongDescription" => {}
            _ => println!("ignoring read_float_parameter_type '{}'", cnode.tag_name().name())
        };
    }

    let mut spt = StringParameterType {
        name: NameDescription::new(name),
        encoding,
    };
    read_name_description(&mut spt.name, node);

    Ok(ParameterType::String(spt))
}


pub(super) fn read_aggregate_parameter_type(mdb: &mut MissionDatabase, path: &QualifiedName, node: &Node) -> Result<(), XtceError> {
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "" => {}
            _ => println!("ignoring read_aggregate_parameter_type '{}'", cnode.tag_name().name())
        };
    }
    Ok(())
}


pub(super) fn read_absolute_time_parameter_type(mdb: &mut MissionDatabase, path: &QualifiedName, node: &Node) -> Result<(), XtceParseError> {
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "Encoding" => {
                //TODO
            }
            "ReferenceTime" => {
                //TODO
            }
            "" => {}
            _ => println!("ignoring read_absolute_time_parameter_type '{}'", cnode.tag_name().name())
        };
    }
    Ok(())
}


fn read_enumeration_list(elist: &mut Vec<EnumeratedValue>, node: &Node) -> Result<(), XtceError> {
    for cnode in node.children().filter(|n| !n.tag_name().name().is_empty()) {
        let value = read_mandatory_attribute::<i64>(&cnode, "value")?;
        let label = read_mandatory_attribute::<String>(&cnode, "label")?;
        let max_value = read_attribute::<i64>(&cnode, "value")?.unwrap_or(value);
        let description = read_attribute::<String>(&cnode, "shortDescription")?;

        elist.push(EnumeratedValue { value, label, max_value, description });
    }
    Ok(())
}
*/

fn read_unit_set(units: &mut Vec<UnitType>, node: &Node) -> Result<(), XtceError> {
    for cnode in node.children() {
        match cnode.tag_name().name() {
            "Unit" => {
                let power = read_attribute::<f64>(&cnode, "power")?.unwrap_or(1f64);
                let factor = read_attribute::<String>(&cnode, "factor")?.unwrap_or("1".to_owned());
                let description = read_attribute::<String>(&cnode, "shortDescription")?;
                let unit = cnode.text().ok_or_else(|| get_parse_error("No unit present".to_owned(), &cnode))?;

                units.push(UnitType { unit: unit.to_owned(), power, factor, description });
            }
            _ => {}
        }
    }
    Ok(())
}
