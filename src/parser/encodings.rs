use std::str::FromStr;

use super::*;

use crate::{bitbuffer::ByteOrder, mdb::{*, types::{IntegerEncodingType, DataEncoding, StringDataEncoding, FloatDataEncoding, FloatEncodingType, IntegerDataEncoding, StringSizeType}}};
use roxmltree::Node;

pub(super) fn read_integer_data_encoding(
    _mdb: &MissionDatabase,
    _path: &QualifiedName,
    node: &Node,
    base_encoding: &DataEncoding,
) -> Result<IntegerDataEncoding, XtceError> {
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
        (read_attribute::<ByteOrder>(node, "referenceLocation")?).unwrap_or(ByteOrder::BigEndian);

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "" => {}
            _ => println!("ignoring read_integer_data_encoding type '{}'", cnode.tag_name().name()),
        };
    }

    Ok(IntegerDataEncoding { size_in_bits, encoding, byte_order })
}

pub(super) fn read_float_data_encoding(
    _mdb: &MissionDatabase,
    _path: &QualifiedName,
    node: &Node,
    base_encoding: &DataEncoding,
) -> Result<FloatDataEncoding, XtceError> {
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

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "" => {}
            _ => println!("ignoring read_integer_data_encoding '{}'", cnode.tag_name().name()),
        };
    }
    Ok(FloatDataEncoding { size_in_bits, encoding })
}

pub(super) fn read_string_data_encoding(
    _mdb: &MissionDatabase,
    _path: &QualifiedName,
    node: &Node,
    base_encoding: &DataEncoding,
) -> Result<StringDataEncoding, XtceError> {
    let encoding = read_attribute::<String>(node, "encoding")?.unwrap_or_else(|| {
        if let DataEncoding::String(sde) = base_encoding {
            sde.encoding.to_owned()
        } else {
            "UTF-8".to_owned()
        }
    });
    let mut size_in_bits = 0;
    let mut termination_char = 0;

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "SizeInBits" => {
                for cnode1 in cnode.children() {
                    match cnode1.tag_name().name() {
                        "Fixed" => {
                            for cnode2 in cnode1.children() {
                                match cnode2.tag_name().name() {
                                    "FixedValue" => {
                                        size_in_bits = read_mandatory_text::<u32>(&cnode2)?;
                                    }
                                    "" => {}
                                    _ => {
                                        return Err(get_parse_error(
                                            format!(
                                                "unsupported Fixed size type {}",
                                                cnode2.tag_name().name()
                                            ),
                                            &cnode2,
                                        )
                                        .into());
                                    }
                                }
                            }
                        }
                        "TerminationChar" => {
                            let hexv = read_mandatory_text::<String>(&cnode1)?;
                            let v = hex::decode(&hexv).or_else(|_e| {
                                return Err(get_parse_error(
                                    format!("Cannot decode string as hex: '{}'", &hexv),
                                    &cnode1,
                                ));
                            })?;
                            if v.len() != 1 {
                                return Err(get_parse_error(
                                    format!("Expected hex byte (2 characters): '{}'", hexv),
                                    &cnode1,
                                )
                                .into());
                            }
                            termination_char = v[0];
                        }
                        "LeadingSize" => {
                            todo!()
                        }
                        "" => {}
                        _ => {
                            return Err(get_parse_error(
                                format!("unsupported size type {}", cnode1.tag_name().name()),
                                &cnode1,
                            )
                            .into());
                        }
                    }
                }
            }
            "" => {}
            _ => println!("ignoring read_string_data_encoding '{}'", cnode.tag_name().name()),
        };
    }
    Ok(StringDataEncoding {
        sizeType: StringSizeType::Fixed,
        size_in_bits,
        sizeInBitsOfSizeTag: 0,
        encoding,
        termination_char,
    })
}

impl FromStr for ByteOrder {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mostSignificantByteFirst" => Ok(ByteOrder::BigEndian),
            "leastSignificantByteFirst" => Ok(ByteOrder::LittleEndian),
            _ => Err("please use one of mostSignificantByteFirst or leastSignificantByteFirst"
                .to_owned()),
        }
    }
}

impl FromStr for IntegerEncodingType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
                "unsigned" => Ok(IntegerEncodingType::Unsigned),
                "signmagnitude" => Ok(IntegerEncodingType::SignMagnitude),
                "twoscomplement" | "twoscompliment" => Ok(IntegerEncodingType::TwosComplement),
                "onescomplement" => Ok(IntegerEncodingType::OnesComplement),
                _ => {
                    Err("please use one of unsigned, signMagnitude, towsComplement, onesComplement".to_owned())
                }
        }
    }
}
