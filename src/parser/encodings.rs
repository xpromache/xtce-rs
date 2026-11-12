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
            BinarySize,
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

    let byte_order = (read_attribute::<ByteOrder>(node, "byteOrder")?).unwrap_or_else(|| {
        if let DataEncoding::Integer(ide) = base_encoding {
            ide.byte_order
        } else {
            ByteOrder::BigEndian
        }
    });
    if size_in_bits == 0 || size_in_bits > 64 {
        return Err(get_parse_error(format!("Invalid integer sizeInBits {}", size_in_bits), node));
    }

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "" | "DefaultCalibrator" | "ContextCalibratorList" => {}
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

    let valid_size = match encoding {
        FloatEncodingType::IEEE754_1985 => matches!(size_in_bits, 16 | 32 | 64),
        FloatEncodingType::Milstd1750a => matches!(size_in_bits, 32 | 48),
    };
    if !valid_size {
        return Err(get_parse_error(
            format!(
                "Invalid size in bits {} for float encoding {:?} (IEEE754_1985 supports 16, 32 or 64, MILSTD_1750A supports 32 or 48)",
                size_in_bits, encoding
            ),
            &node,
        ));
    }

    let byte_order =
    (read_attribute::<ByteOrder>(node, "byteOrder")?).unwrap_or(ByteOrder::BigEndian);

    for cnode in node.children() {
        match cnode.tag_name().name() {
            "" | "DefaultCalibrator" | "ContextCalibratorList" => {}
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
    if !crate::proc::encodings::is_supported_charset(&encoding) {
        return Err(get_parse_error(format!("Unsupported string encoding '{}'", encoding), &node));
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
    let mut size_in_bits = match base_encoding {
        DataEncoding::Binary(bde) => match &bde.size_in_bits {
            BinarySize::Fixed(x) => Some(BinarySize::Fixed(*x)),
            BinarySize::LeadingSize(x) => Some(BinarySize::LeadingSize(*x)),
            _ => None,
        },
        _ => None,
    };
    let mut fixed_minus_one = false;
    let mut algorithm = None;

    for cnode in children(&node) {
        match cnode.tag_name().name() {
            "SizeInBits" => {
                // Yamcs extension: <LeadingSize sizeInBitsOfSizeTag="x"/> inside SizeInBits
                if let Some(lsnode) = children(&cnode).find(|n| n.tag_name().name() == "LeadingSize") {
                    let tag_bits = read_attribute::<u32>(&lsnode, "sizeInBitsOfSizeTag")?.unwrap_or(16);
                    check_size_tag(tag_bits, &lsnode)?;
                    size_in_bits = Some(BinarySize::LeadingSize(tag_bits));
                    continue;
                }
                match read_integer_value(mdb, ctx, &cnode)? {
                    IntegerValue::FixedValue(-1) => fixed_minus_one = true,
                    IntegerValue::FixedValue(v) => {
                        let v = u32::try_from(v).map_err(|_| {
                            get_parse_error(format!("Invalid binary size in bits {}", v), &cnode)
                        })?;
                        size_in_bits = Some(BinarySize::Fixed(v));
                    }
                    IntegerValue::DynamicValue(dv) => size_in_bits = Some(BinarySize::Dynamic(dv)),
                }
            }
            "FromBinaryTransformAlgorithm" => {
                let text = children(&cnode)
                    .find(|n| n.tag_name().name() == "AlgorithmText")
                    .and_then(|n| n.text())
                    .unwrap_or("")
                    .trim()
                    .to_owned();
                algorithm = Some(text);
            }
            _ => log::warn!(
                "Ignoring unsupported element {} for binary data encoding",
                cnode.tag_name().name()
            ),
        }
    }

    if let Some(algo) = algorithm {
        // the Yamcs LeadingSizeBinaryDecoder(n) algorithm is supported natively
        size_in_bits = Some(match parse_leading_size_decoder(&algo) {
            Some(tag_bits) => {
                check_size_tag(tag_bits, node)?;
                BinarySize::LeadingSize(tag_bits)
            }
            None => {
                log::warn!(
                    "Binary decoding algorithm '{}' not supported; parameters using it cannot be decoded",
                    algo
                );
                BinarySize::Custom(algo)
            }
        });
    } else if fixed_minus_one {
        return Err(get_parse_error(
            "Binary size in bits -1 requires a FromBinaryTransformAlgorithm",
            node,
        ));
    }

    match size_in_bits {
        Some(size_in_bits) => Ok(BinaryDataEncoding { size_in_bits }),
        None => Err(get_parse_error("Binary data encoding without size in bits", node)),
    }
}

fn check_size_tag(tag_bits: u32, node: &Node) -> Result<()> {
    if tag_bits == 0 || tag_bits > 32 {
        Err(get_parse_error(
            format!("Invalid size of the size tag {} bits; should be between 1 and 32", tag_bits),
            node,
        ))
    } else {
        Ok(())
    }
}

/// recognizes the Yamcs algorithm text "org.yamcs.algo.LeadingSizeBinaryDecoder(n)" and returns n
/// (without arguments, the default is 16)
fn parse_leading_size_decoder(text: &str) -> Option<u32> {
    let text = text.trim();
    let rest = text
        .strip_prefix("org.yamcs.algo.LeadingSizeBinaryDecoder")
        .or_else(|| text.strip_prefix("LeadingSizeBinaryDecoder"))?
        .trim();
    if rest.is_empty() || rest == "()" {
        return Some(16);
    }
    rest.strip_prefix('(')?.strip_suffix(')')?.trim().parse().ok()
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
