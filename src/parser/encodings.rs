use std::str::FromStr;

use super::{
    misc::{read_dynamic_value, read_integer_value},
    *,
};

use crate::{
    bitbuffer::ByteOrder,
    mdb::{
        types::{
            DataEncoding, FloatDataEncoding, FloatEncodingType, IntegerDataEncoding,
            IntegerEncodingType, StringBoxSize, StringDataEncoding, StringSize, BinaryDataEncoding,
        },
        *,
    },
};
use roxmltree::Node;

pub(super) fn read_integer_data_encoding(
    _mdb: &MissionDatabase,
    _path: &QualifiedName,
    node: &Node,
    base_encoding: &DataEncoding,
) -> Result<IntegerDataEncoding> {
    //  println!("integer_data_encoding: {:?}", node);
    let size_in_bits = read_attribute::<u8>(node, "sizeInBits")?.unwrap_or_else(|| {
        if let DataEncoding::Integer(ide) = base_encoding {
            ide.size_in_bits
        } else {
            8
        }
    });

    let encoding = read_attribute::<IntegerEncodingType>(node, "encoding")?.unwrap_or_else(|| {
        if let DataEncoding::Integer(ide) = base_encoding {
            ide.encoding
        } else {
            IntegerEncodingType::Unsigned
        }
    });

    let byte_order =
        (read_attribute::<ByteOrder>(node, "byteOrder")?).unwrap_or(ByteOrder::BigEndian);

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "" => {}
            _ => log::warn!(
                "ignoring integer data encoding unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }

    Ok(IntegerDataEncoding { size_in_bits, encoding, byte_order })
}

pub(super) fn read_float_data_encoding(
    _mdb: &MissionDatabase,
    _path: &QualifiedName,
    node: &Node,
    base_encoding: &DataEncoding,
) -> Result<FloatDataEncoding> {
    let size_in_bits = read_attribute::<u8>(node, "sizeInBits")?.unwrap_or_else(|| {
        if let DataEncoding::Float(fde) = base_encoding {
            fde.size_in_bits
        } else {
            32
        }
    });
    if size_in_bits != 32 && size_in_bits != 64 {
        return Err(get_parse_error(
            format!("Invalid size in bits {}, should be 32 or 64", size_in_bits),
            &node,
        )
        .into());
    }
    let encoding;

    if let Some(encs) = node.attribute("encoding") {
        encoding = match encs {
            "IEEE754_1985" | "IEEE754" => FloatEncodingType::IEEE754_1985,
            "MILSTD_1750A" => FloatEncodingType::Milstd1750a,
            _ => {
                return Err(get_parse_error(
                    "Invalid float encoding type '".to_owned() + encs + "'",
                    &node,
                )
                .into())
            }
        };
    } else if let DataEncoding::Float(fde) = base_encoding {
        encoding = fde.encoding;
    } else {
        encoding = FloatEncodingType::IEEE754_1985;
    }

    let byte_order =
    (read_attribute::<ByteOrder>(node, "byteOrder")?).unwrap_or(ByteOrder::BigEndian);

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "" => {}
            _ => log::warn!(
                "ignoring float data encoding unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }
    Ok(FloatDataEncoding { size_in_bits, encoding, byte_order })
}

pub(super) fn read_string_data_encoding(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
    base_encoding: &DataEncoding,
) -> Result<StringDataEncoding> {
    let encoding = read_attribute::<String>(node, "encoding")?.unwrap_or_else(|| {
        if let DataEncoding::String(sde) = base_encoding {
            sde.encoding.to_owned()
        } else {
            "UTF-8".to_owned()
        }
    });
    let mut size_in_bits = None;

    let mut max_box_size_in_bytes = None;
    let mut box_size_in_bits = StringBoxSize::Undefined;

    for cnode in children(&node) {
        match cnode.tag_name().name() {
            "SizeInBits" => {
                for cnode1 in children(&cnode) {
                    match cnode1.tag_name().name() {
                        "Fixed" => {
                            for cnode2 in children(&cnode1) {
                                match cnode2.tag_name().name() {
                                    "FixedValue" => {
                                        let size = read_mandatory_text::<u32>(&cnode2)?;
                                        box_size_in_bits = StringBoxSize::Fixed(size);
                                        size_in_bits = Some(StringSize::Fixed(size));
                                    }
                                    _ => {
                                        return Err(unsupported("size type", &cnode2));
                                    }
                                }
                            }
                        }
                        "TerminationChar" => {
                            size_in_bits.replace(StringSize::TerminationChar(
                                parse_terminator_char(&cnode1)?,
                            ));
                        }
                        "LeadingSize" => {
                            size_in_bits.replace(StringSize::LeadingSize(
                                parse_leading_size(&cnode1)?
                            ));
                        }
                        _ => {
                            return Err(unsupported("size type", &cnode1));
                        }
                    }
                }
            }
            "Variable" => {
                let msb = read_mandatory_attribute::<u32>(&cnode, "maxSizeInBits")?;
                max_box_size_in_bytes.replace(msb / 8);
                for cnode1 in children(&cnode) {
                    match cnode1.tag_name().name() {
                        "TerminationChar" => {
                            size_in_bits.replace(StringSize::TerminationChar(
                                parse_terminator_char(&cnode1)?,
                            ));
                        }
                        "LeadingSize" => {                            
                            size_in_bits.replace(StringSize::LeadingSize(
                                parse_leading_size(&cnode1)?
                            ));
                        }
                        "DynamicValue" => {
                            let dv = read_dynamic_value(mdb, ctx, &cnode1, true)?;
                            if dv.para_ref.pidx != INVALID_PARAM_IDX {
                                box_size_in_bits = StringBoxSize::Dynamic(dv);
                            }
                        }

                        _ => return Err(unsupported("size type", &cnode1)),
                    }
                }
            }
            _ => log::warn!(
                "ignoring string data encoding unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }

    if size_in_bits.is_none() {
        return Err(get_parse_error("Size in bits not specified", &node).into());
    }

    Ok(StringDataEncoding {
        encoding,
        max_box_size_in_bytes,
        size_in_bits: size_in_bits.unwrap(),
        box_size_in_bits,
    })
}



pub(super) fn read_binary_data_encoding(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
    base_encoding: &DataEncoding,
) -> Result<BinaryDataEncoding> {
    for cnode in children(&node) {
        match cnode.tag_name().name() {
            "SizeInBits" => {
                let iv = read_integer_value(mdb, ctx, &cnode)?;
            }
            _ => log::warn!("Ignorng unsupported element {} for binary data encoding", cnode.tag_name().name())
        }
    }
todo!()
   // Ok(BinaryDataEncoding{})
}


fn parse_leading_size(node: &Node) -> Result<u32> {
    let v = read_attribute::<u32>(&node, "sizeInBitsOfSizeTag")?
    .unwrap_or(16);

    if v%8 !=0 {
        Err(get_parse_error(format!("Invalid value {} for sizeInBitsOfSizeTag; only multiples of 8 are supported'", v), node))?
    } else {
        Ok(v/8)
    }

}
fn parse_terminator_char(node: &Node) -> Result<u8> {
    let hexv = read_mandatory_text::<String>(node)?;
    let v = hex::decode(&hexv).or_else(|_e| {
        return Err(get_parse_error(format!("Cannot decode string as hex: '{}'", &hexv), node));
    })?;
    if v.len() != 1 {
        return Err(
            get_parse_error(format!("Expected hex byte (2 characters): '{}'", hexv), node).into()
        );
    }
    Ok(v[0])
}

impl FromStr for ByteOrder {
    type Err = XtceError;

    fn from_str(s: &str) -> Result<Self> {
        match s {
            "mostSignificantByteFirst" => Ok(ByteOrder::BigEndian),
            "leastSignificantByteFirst" => Ok(ByteOrder::LittleEndian),
            _ => Err(XtceError::InvalidValue("please use one of mostSignificantByteFirst or leastSignificantByteFirst"
                .to_owned())),
        }
    }
}

impl FromStr for IntegerEncodingType {
    type Err = XtceError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "unsigned" => Ok(IntegerEncodingType::Unsigned),
            "signmagnitude" => Ok(IntegerEncodingType::SignMagnitude),
            "twoscomplement" | "twoscompliment" => Ok(IntegerEncodingType::TwosComplement),
            "onescomplement" => Ok(IntegerEncodingType::OnesComplement),
            _ => Err(XtceError::InvalidValue("please use one of unsigned, signMagnitude, towsComplement, onesComplement"
                .to_owned())),
        }
    }
}
