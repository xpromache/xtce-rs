use roxmltree::Node;

use super::{
    misc::{read_integer_value, read_match_criteria, read_para_insta_ref, resolve_ref},
    *,
};
use encodings::*;

use crate::mdb::{
    types::{
        AbsoluteTimeDataType, AggregateDataType, AlarmLevel, AlarmRanges, ArrayDataType,
        BinaryDataType, BooleanDataType, Calibrator, ContextCalibrator, DataEncoding, DataType,
        EnumeratedDataType, EnumerationAlarm, EnumerationContextAlarm, FloatDataType,
        IntegerDataType, MathOpElement, MathOperator, Member, NumericAlarm, NumericContextAlarm,
        NumericRange, SplineCalibrator, SplinePoint, StringDataType, TimeEpoch, TimeReference,
        TypeData, ValidRange, ValueEnumeration,
    },
    *,
};

pub(super) fn add_parameter_type(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<()> {
    let tag = ctx.node.tag_name().name();
    let dtype = match tag {
        "IntegerParameterType"
        | "FloatParameterType"
        | "EnumeratedParameterType"
        | "BooleanParameterType"
        | "StringParameterType"
        | "BinaryParameterType"
        | "AbsoluteTimeParameterType" => read_base_data_type(mdb, ctx, tag)?,
        "AggregateParameterType" => read_aggregate_parameter_type(mdb, ctx)?,
        "ArrayParameterType" => read_array_parameter_type(mdb, ctx)?,
        _ => {
            log::warn!("ignoring parameter type '{}'", tag);
            return Ok(());
        }
    };

    mdb.add_parameter_type(ctx.path, dtype);
    Ok(())
}

/// the default type data (no base type) corresponding to the XTCE element
fn default_type_data(tag: &str) -> TypeData {
    match tag {
        "IntegerParameterType" => TypeData::Integer(IntegerDataType {
            size_in_bits: 32,
            signed: true,
            default_alarm: None,
            context_alarm: vec![],
        }),
        "FloatParameterType" => TypeData::Float(FloatDataType {
            size_in_bits: 32,
            default_alarm: None,
            context_alarm: vec![],
        }),
        "EnumeratedParameterType" => TypeData::Enumerated(EnumeratedDataType {
            enumeration: vec![],
            default_alarm: None,
            context_alarm: vec![],
        }),
        "BooleanParameterType" => TypeData::Boolean(BooleanDataType {
            one_string_value: "True".to_owned(),
            zero_string_value: "False".to_owned(),
        }),
        "StringParameterType" => TypeData::String(StringDataType {}),
        "BinaryParameterType" => TypeData::Binary(BinaryDataType { size_in_bits: 0 }),
        "AbsoluteTimeParameterType" => TypeData::AbsoluteTime(AbsoluteTimeDataType {
            scale: 1.0,
            offset: 0.0,
            reference: TimeReference::None,
        }),
        _ => unreachable!("not a base type: {}", tag),
    }
}

fn same_kind(a: &TypeData, b: &TypeData) -> bool {
    std::mem::discriminant(a) == std::mem::discriminant(b)
}

/// reads integer, float, enumerated, boolean, string, binary and absolute time types.
///
/// If the type has a baseType attribute, the definition of the base type is used as default
/// and the elements/attributes present in this type override it.
fn read_base_data_type(mdb: &mut MissionDatabase, ctx: &ParseContext, tag: &str) -> Result<DataType> {
    let node = ctx.node;
    let type_data0 = default_type_data(tag);

    let mut dtype = match node.attribute("baseType") {
        Some(base_ref) => {
            let base_idx = resolve_ref(mdb, ctx, base_ref, NameReferenceType::ParameterType)?;
            let base = mdb.get_data_type(base_idx);
            if !same_kind(&base.type_data, &type_data0) {
                return Err(get_parse_error(
                    format!("Base type '{}' is not of the same kind as {}", base_ref, tag),
                    &node,
                ));
            }
            base.clone()
        }
        None => DataType::new(NameDescription::new(ctx.name), type_data0),
    };
    dtype.ndescr = read_name_description(ctx);

    // attributes
    match &mut dtype.type_data {
        TypeData::Integer(idt) => {
            if let Some(signed) = read_attribute::<bool>(&node, "signed")? {
                idt.signed = signed;
            }
            if let Some(size) = read_attribute::<u32>(&node, "sizeInBits")? {
                if size == 0 || size > 64 {
                    return Err(get_parse_error(format!("Invalid sizeInBits {}", size), &node));
                }
                idt.size_in_bits = size;
            }
        }
        TypeData::Float(fdt) => {
            if let Some(size) = read_attribute::<u32>(&node, "sizeInBits")? {
                if size != 32 && size != 64 && size != 128 {
                    return Err(get_parse_error(
                        format!("Invalid sizeInBits {} for float type; use 32, 64 or 128", size),
                        &node,
                    ));
                }
                fdt.size_in_bits = size;
            }
        }
        TypeData::Boolean(bdt) => {
            if let Some(v) = read_attribute::<String>(&node, "oneStringValue")? {
                bdt.one_string_value = v;
            }
            if let Some(v) = read_attribute::<String>(&node, "zeroStringValue")? {
                bdt.zero_string_value = v;
            }
        }
        _ => {}
    }

    for cnode in children(&node) {
        match cnode.tag_name().name() {
            "IntegerDataEncoding" | "FloatDataEncoding" | "StringDataEncoding"
            | "BinaryDataEncoding" => {
                read_data_encoding(mdb, ctx, &cnode, &mut dtype)?;
            }
            "Encoding" if tag == "AbsoluteTimeParameterType" => {
                read_time_encoding(mdb, ctx, &cnode, &mut dtype)?;
            }
            "ReferenceTime" if tag == "AbsoluteTimeParameterType" => {
                let reference = read_reference_time(mdb, ctx, &cnode)?;
                if let TypeData::AbsoluteTime(atd) = &mut dtype.type_data {
                    atd.reference = reference;
                }
            }
            "UnitSet" => dtype.units = read_unit_set(&cnode)?,
            "ValidRange" => dtype.valid_range = Some(read_valid_range(&cnode)?),
            "EnumerationList" => {
                if let TypeData::Enumerated(edt) = &mut dtype.type_data {
                    edt.enumeration = read_enumeration_list(&cnode)?;
                } else {
                    return Err(unsupported("element", &cnode));
                }
            }
            "DefaultAlarm" => read_default_alarm(&cnode, &mut dtype.type_data)?,
            "ContextAlarmList" => read_context_alarm_list(mdb, ctx, &cnode, &mut dtype.type_data)?,
            "LongDescription" | "AliasSet" | "AncillaryDataSet" => {}
            // validation of string sizes, relevant for commands
            "SizeRangeInCharacters" => {}
            _ => log::warn!(
                "ignoring {} unknown property '{}'",
                tag,
                cnode.tag_name().name()
            ),
        };
    }

    Ok(dtype)
}

/// reads one of the data encoding elements and sets it (together with calibrators) into the data type
fn read_data_encoding(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
    dtype: &mut DataType,
) -> Result<()> {
    let base = &dtype.encoding;
    let encoding = match node.tag_name().name() {
        "IntegerDataEncoding" => {
            DataEncoding::Integer(read_integer_data_encoding(mdb, ctx.path, node, base)?)
        }
        "FloatDataEncoding" => {
            DataEncoding::Float(read_float_data_encoding(mdb, ctx.path, node, base)?)
        }
        "StringDataEncoding" => {
            DataEncoding::String(read_string_data_encoding(mdb, ctx, node, base)?)
        }
        "BinaryDataEncoding" => {
            DataEncoding::Binary(read_binary_data_encoding(mdb, ctx, node, base)?)
        }
        _ => return Err(unsupported("encoding", node)),
    };
    dtype.encoding = encoding;

    for cnode in children(node) {
        match cnode.tag_name().name() {
            "DefaultCalibrator" => dtype.calibrator = Some(read_calibrator(&cnode)?),
            "ContextCalibratorList" => {
                dtype.context_calibrators = read_context_calibrator_list(mdb, ctx, &cnode)?
            }
            _ => {}
        }
    }
    Ok(())
}

/// reads the Encoding element of the absolute time parameter type
fn read_time_encoding(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
    dtype: &mut DataType,
) -> Result<()> {
    if let Some(units) = node.attribute("units") {
        if units != "seconds" {
            return Err(get_parse_error(
                format!("Unsupported time encoding units '{}'; only 'seconds' is supported", units),
                node,
            ));
        }
    }
    let scale = read_attribute::<f64>(node, "scale")?;
    let offset = read_attribute::<f64>(node, "offset")?;
    if let TypeData::AbsoluteTime(atd) = &mut dtype.type_data {
        if let Some(scale) = scale {
            atd.scale = scale;
        }
        if let Some(offset) = offset {
            atd.offset = offset;
        }
    }
    for cnode in children(node) {
        read_data_encoding(mdb, ctx, &cnode, dtype)?;
    }
    Ok(())
}

fn read_reference_time(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<TimeReference> {
    let cnode = children(node)
        .next()
        .ok_or_else(|| get_parse_error("ReferenceTime without Epoch or OffsetFrom", node))?;
    match cnode.tag_name().name() {
        "Epoch" => {
            let text = read_mandatory_text::<String>(&cnode)?;
            let epoch = match text.trim() {
                "TAI" => TimeEpoch::Tai,
                "J2000" => TimeEpoch::J2000,
                "UNIX" | "POSIX" => TimeEpoch::Unix,
                "GPS" => TimeEpoch::Gps,
                s => TimeEpoch::DateTime(crate::time::parse(s).ok_or_else(|| {
                    get_parse_error(format!("cannot parse epoch date '{}'", s), &cnode)
                })?),
            };
            Ok(TimeReference::Epoch(epoch))
        }
        "OffsetFrom" => Ok(TimeReference::OffsetFrom(read_para_insta_ref(mdb, ctx, &cnode, false)?)),
        _ => Err(unsupported("reference time", &cnode)),
    }
}

fn read_calibrator(node: &Node) -> Result<Calibrator> {
    let cnode = children(node)
        .next()
        .ok_or_else(|| get_parse_error("No calibrator specified", node))?;
    let cal = match cnode.tag_name().name() {
        "PolynomialCalibrator" => read_polynomial_calibrator(&cnode)?,
        "SplineCalibrator" => read_spline_calibrator(&cnode)?,
        "MathOperationCalibrator" => read_math_operation_calibrator(&cnode)?,
        name => {
            log::warn!("Unsupported calibrator {}; the parameters using it cannot be calibrated", name);
            Calibrator::Unsupported(name.to_owned())
        }
    };
    Ok(cal)
}

fn read_polynomial_calibrator(node: &Node) -> Result<Calibrator> {
    let mut coefficients = Vec::new();
    for cnode in children(node) {
        match cnode.tag_name().name() {
            "Term" => {
                let exponent = read_mandatory_attribute::<u32>(&cnode, "exponent")? as usize;
                let coefficient = read_mandatory_attribute::<f64>(&cnode, "coefficient")?;
                if exponent > 32 {
                    return Err(get_parse_error(
                        format!("Polynomial exponent {} too large", exponent),
                        &cnode,
                    ));
                }
                if coefficients.len() <= exponent {
                    coefficients.resize(exponent + 1, 0.0);
                }
                coefficients[exponent] += coefficient;
            }
            _ => return Err(unsupported("polynomial calibrator element", &cnode)),
        }
    }
    Ok(Calibrator::Polynomial(coefficients))
}

fn read_spline_calibrator(node: &Node) -> Result<Calibrator> {
    let order = read_attribute::<u32>(node, "order")?.unwrap_or(1);
    let extrapolate = read_attribute::<bool>(node, "extrapolate")?.unwrap_or(false);
    if order != 1 || !extrapolate {
        log::debug!(
            "Spline calibrator with order={} extrapolate={}: like Yamcs, linear interpolation with extrapolation is used",
            order,
            extrapolate
        );
    }
    let mut points = Vec::new();
    for cnode in children(node) {
        match cnode.tag_name().name() {
            "SplinePoint" => points.push(SplinePoint {
                raw: read_mandatory_attribute::<f64>(&cnode, "raw")?,
                calibrated: read_mandatory_attribute::<f64>(&cnode, "calibrated")?,
            }),
            _ => return Err(unsupported("spline calibrator element", &cnode)),
        }
    }
    if points.len() < 2 {
        return Err(get_parse_error("Spline calibrator requires at least two points", node));
    }
    points.sort_by(|a, b| a.raw.total_cmp(&b.raw));

    Ok(Calibrator::Spline(SplineCalibrator { order, extrapolate, points }))
}

fn read_math_operation_calibrator(node: &Node) -> Result<Calibrator> {
    let elements = read_math_operation(node, &mut |cnode| {
        Err(get_parse_error("Parameter references are not supported in MathOperationCalibrator", cnode))
    })?;
    Ok(Calibrator::MathOperation(elements))
}

/// reads the elements of a math operation in reverse polish notation and verifies that the expression is valid.
/// The ParameterInstanceRefOperand elements are converted using the given function; TriggerSet elements are skipped.
pub(super) fn read_math_operation(
    node: &Node,
    param_operand: &mut dyn FnMut(&Node) -> Result<MathOpElement>,
) -> Result<Vec<MathOpElement>> {
    let mut elements = Vec::new();
    let mut depth: usize = 0;
    for cnode in children(node) {
        let el = match cnode.tag_name().name() {
            "TriggerSet" => continue,
            "ValueOperand" => MathOpElement::Value(read_mandatory_text::<f64>(&cnode)?),
            "ThisParameterOperand" => MathOpElement::ThisParameter,
            "Operator" => {
                let s = read_mandatory_text::<String>(&cnode)?;
                let op = MathOperator::from_xtce_name(s.trim()).ok_or_else(|| {
                    get_parse_error(format!("Unknown math operator '{}'", s), &cnode)
                })?;
                MathOpElement::Operator(op)
            }
            "ParameterInstanceRefOperand" => param_operand(&cnode)?,
            _ => return Err(unsupported("math operation element", &cnode)),
        };
        // check that the expression is valid
        depth = match &el {
            MathOpElement::Operator(op) => {
                let (consumed, produced) = op.stack_effect();
                if depth < consumed {
                    return Err(get_parse_error(
                        format!("Not enough operands for operator {:?}", op),
                        &cnode,
                    ));
                }
                depth - consumed + produced
            }
            _ => depth + 1,
        };
        elements.push(el);
    }
    if depth != 1 {
        return Err(get_parse_error(
            format!("Invalid math operation: the evaluation stack has {} elements at the end instead of 1", depth),
            node,
        ));
    }
    Ok(elements)
}

fn read_context_calibrator_list(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<Vec<ContextCalibrator>> {
    let mut r = Vec::new();
    for cnode in children(node) {
        if cnode.tag_name().name() != "ContextCalibrator" {
            return Err(unsupported("element", &cnode));
        }
        let mut context = None;
        let mut calibrator = None;
        for ccnode in children(&cnode) {
            match ccnode.tag_name().name() {
                "ContextMatch" => context = Some(read_match_criteria(mdb, ctx, &ccnode)?),
                "Calibrator" => calibrator = Some(read_calibrator(&ccnode)?),
                _ => return Err(unsupported("element", &ccnode)),
            }
        }
        match (context, calibrator) {
            (Some(context), Some(calibrator)) => r.push(ContextCalibrator { context, calibrator }),
            _ => return Err(get_parse_error("ContextCalibrator requires ContextMatch and Calibrator", &cnode)),
        }
    }
    Ok(r)
}

fn read_numeric_range(node: &Node) -> Result<NumericRange> {
    Ok(NumericRange {
        min_inclusive: read_attribute::<f64>(node, "minInclusive")?,
        min_exclusive: read_attribute::<f64>(node, "minExclusive")?,
        max_inclusive: read_attribute::<f64>(node, "maxInclusive")?,
        max_exclusive: read_attribute::<f64>(node, "maxExclusive")?,
    })
}

fn read_valid_range(node: &Node) -> Result<ValidRange> {
    Ok(ValidRange {
        range: read_numeric_range(node)?,
        applies_to_calibrated: read_attribute::<bool>(node, "validRangeAppliesToCalibrated")?
            .unwrap_or(true),
    })
}

fn read_alarm_ranges(node: &Node) -> Result<AlarmRanges> {
    let mut r = AlarmRanges::default();
    for cnode in children(node) {
        let range = Some(read_numeric_range(&cnode)?);
        match cnode.tag_name().name() {
            "WatchRange" => r.watch = range,
            "WarningRange" => r.warning = range,
            "DistressRange" => r.distress = range,
            "CriticalRange" => r.critical = range,
            "SevereRange" => r.severe = range,
            _ => return Err(unsupported("alarm range", &cnode)),
        }
    }
    Ok(r)
}

/// reads the content of a numeric DefaultAlarm or ContextAlarm (the ContextMatch is skipped)
fn read_numeric_alarm(node: &Node) -> Result<NumericAlarm> {
    let min_violations = read_attribute::<u32>(node, "minViolations")?.unwrap_or(1);
    let mut static_alarm_ranges = AlarmRanges::default();
    for cnode in children(node) {
        match cnode.tag_name().name() {
            "StaticAlarmRanges" => static_alarm_ranges = read_alarm_ranges(&cnode)?,
            "ContextMatch" => {}
            name => log::warn!("ignoring unsupported alarm element {}", name),
        }
    }
    Ok(NumericAlarm { min_violations, static_alarm_ranges })
}

impl std::str::FromStr for AlarmLevel {
    type Err = XtceError;

    fn from_str(s: &str) -> Result<Self> {
        match s.to_lowercase().as_str() {
            "normal" => Ok(AlarmLevel::Normal),
            "watch" => Ok(AlarmLevel::Watch),
            "warning" => Ok(AlarmLevel::Warning),
            "distress" => Ok(AlarmLevel::Distress),
            "critical" => Ok(AlarmLevel::Critical),
            "severe" => Ok(AlarmLevel::Severe),
            _ => Err(XtceError::InvalidValue(format!("Invalid alarm level '{}'", s))),
        }
    }
}

fn read_enumeration_alarm(node: &Node) -> Result<EnumerationAlarm> {
    let min_violations = read_attribute::<u32>(node, "minViolations")?.unwrap_or(1);
    let default_level =
        read_attribute::<AlarmLevel>(node, "defaultAlarmLevel")?.unwrap_or(AlarmLevel::Normal);
    let mut alarms = Vec::new();
    for cnode in children(node) {
        match cnode.tag_name().name() {
            "EnumerationAlarmList" => {
                for anode in children(&cnode) {
                    let label = read_mandatory_attribute::<String>(&anode, "enumerationLabel")?;
                    let level = read_mandatory_attribute::<AlarmLevel>(&anode, "alarmLevel")?;
                    alarms.push((label, level));
                }
            }
            "ContextMatch" => {}
            name => log::warn!("ignoring unsupported alarm element {}", name),
        }
    }
    Ok(EnumerationAlarm { min_violations, default_level, alarms })
}

fn read_default_alarm(node: &Node, type_data: &mut TypeData) -> Result<()> {
    match type_data {
        TypeData::Integer(idt) => idt.default_alarm = Some(read_numeric_alarm(node)?),
        TypeData::Float(fdt) => fdt.default_alarm = Some(read_numeric_alarm(node)?),
        TypeData::Enumerated(edt) => edt.default_alarm = Some(read_enumeration_alarm(node)?),
        _ => log::warn!("ignoring DefaultAlarm for {:?}", type_data),
    }
    Ok(())
}

fn read_context_alarm_list(
    mdb: &mut MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
    type_data: &mut TypeData,
) -> Result<()> {
    let mut numeric = Vec::new();
    let mut enumerated = Vec::new();
    for cnode in children(node) {
        if cnode.tag_name().name() != "ContextAlarm" {
            return Err(unsupported("element", &cnode));
        }
        let cm = children(&cnode)
            .find(|n| n.tag_name().name() == "ContextMatch")
            .ok_or_else(|| missing("ContextMatch", &cnode))?;
        let context = read_match_criteria(mdb, ctx, &cm)?;
        match type_data {
            TypeData::Integer(_) | TypeData::Float(_) => {
                numeric.push(NumericContextAlarm { context, alarm: read_numeric_alarm(&cnode)? })
            }
            TypeData::Enumerated(_) => enumerated
                .push(EnumerationContextAlarm { context, alarm: read_enumeration_alarm(&cnode)? }),
            _ => log::warn!("ignoring ContextAlarm for {:?}", type_data),
        }
    }
    match type_data {
        TypeData::Integer(idt) => idt.context_alarm = numeric,
        TypeData::Float(fdt) => fdt.context_alarm = numeric,
        TypeData::Enumerated(edt) => edt.context_alarm = enumerated,
        _ => {}
    }
    Ok(())
}

//reads an aggregate parameter type from the XTCE
fn read_aggregate_parameter_type(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<DataType> {
    let mut members = Vec::new();

    for cnode in children(&ctx.node) {
        match cnode.tag_name().name() {
            "MemberList" => {
                for mnode in children(&cnode) {
                    match mnode.tag_name().name() {
                        "Member" => members.push(read_member(mdb, ctx, &mnode)?),
                        _ => log::warn!(
                            "ignoring member list unknown property '{}'",
                            mnode.tag_name().name()
                        ),
                    }
                }
            }
            "LongDescription" | "AliasSet" | "AncillaryDataSet" | "UnitSet" => {}
            _ => log::warn!(
                "ignoring aggregate parameter type unknown property '{}'",
                cnode.tag_name().name()
            ),
        };
    }
    Ok(DataType::new(read_name_description(ctx), TypeData::Aggregate(AggregateDataType { members })))
}

// reads a member of an aggregate type from the XTCE
fn read_member(mdb: &mut MissionDatabase, ctx: &ParseContext, node: &Node) -> Result<Member> {
    let ptype_str = read_mandatory_attribute::<String>(node, "typeRef")?;
    let dtype = resolve_ref(mdb, ctx, &ptype_str, NameReferenceType::ParameterType)?;
    let name_str = read_mandatory_name(node)?;
    let name = mdb.get_or_intern(name_str);
    let ctx1 = ParseContext { name_tree: ctx.name_tree, node: *node, path: ctx.path, name, rtype: ctx.rtype, history: ctx.history };
    let ndescr = read_name_description(&ctx1);

    Ok(Member { ndescr, dtype })
}

fn read_array_parameter_type(mdb: &mut MissionDatabase, ctx: &ParseContext) -> Result<DataType> {
    let node = &ctx.node;
    let ptype_str = read_mandatory_attribute::<String>(node, "arrayTypeRef")?;
    let dtype = resolve_ref(mdb, ctx, &ptype_str, NameReferenceType::ParameterType)?;
    let mut number_of_dimensions = read_attribute::<usize>(node, "numberOfDimensions")?;
    let mut dim = Vec::new();

    for cnode in children(node) {
        match cnode.tag_name().name() {
            "DimensionList" => {
                dim = read_dimension_list(mdb, ctx, &cnode)?;
                if let Some(n) = number_of_dimensions {
                    if n != dim.len() {
                        return Err(get_parse_error(
                            format!("numberOfDimensions={} but {} dimensions specified", n, dim.len()),
                            &cnode,
                        ));
                    }
                }
                number_of_dimensions = Some(dim.len());
            }
            "LongDescription" | "AliasSet" | "AncillaryDataSet" | "UnitSet" => {}
            _ => log::warn!("ignoring array parameter type unknown property '{}'", cnode.tag_name().name()),
        }
    }
    let number_of_dimensions = number_of_dimensions.ok_or_else(|| {
        get_parse_error("Array type requires either numberOfDimensions or DimensionList", node)
    })?;
    if number_of_dimensions == 0 {
        return Err(get_parse_error("Array type with 0 dimensions", node));
    }

    let apt = ArrayDataType { dtype, number_of_dimensions, dim };
    Ok(DataType::new(read_name_description(ctx), TypeData::Array(apt)))
}

/// reads a dimension list and returns the size of each dimension.
///
/// Two forms are supported:
///  - <Dimension><StartingIndex/><EndingIndex/></Dimension> (XTCE 1.2); like in Yamcs the starting index has to be 0
///  - <Size/> (used in some XTCE 1.1 files)
pub(super) fn read_dimension_list(
    mdb: &MissionDatabase,
    ctx: &ParseContext,
    node: &Node,
) -> Result<Vec<IntegerValue>> {
    let mut r = Vec::new();
    for cnode in children(node) {
        match cnode.tag_name().name() {
            "Size" => r.push(read_integer_value(mdb, ctx, &cnode)?),
            "Dimension" => {
                let mut start = None;
                let mut end = None;
                for inode in children(&cnode) {
                    match inode.tag_name().name() {
                        "StartingIndex" => start = Some(read_integer_value(mdb, ctx, &inode)?),
                        "EndingIndex" => end = Some(read_integer_value(mdb, ctx, &inode)?),
                        _ => return Err(unsupported("dimension element", &inode)),
                    }
                }
                match (start, end) {
                    (Some(IntegerValue::FixedValue(0)), Some(end)) => r.push(match end {
                        IntegerValue::FixedValue(v) => IntegerValue::FixedValue(v + 1),
                        IntegerValue::DynamicValue(mut dv) => {
                            match &mut dv.adjustment {
                                Some(adj) => adj.intercept += 1.0,
                                None => {
                                    dv.adjustment =
                                        Some(LinearAdjustment { slope: 1.0, intercept: 1.0 })
                                }
                            }
                            IntegerValue::DynamicValue(dv)
                        }
                    }),
                    _ => {
                        return Err(get_parse_error(
                            "Dimension indexes must be specified with a StartingIndex FixedValue of 0 and an EndingIndex",
                            &cnode,
                        ))
                    }
                }
            }
            _ => return Err(unsupported("dimension list element", &cnode)),
        }
    }
    if r.is_empty() {
        return Err(get_parse_error("Empty dimension list", node));
    }
    Ok(r)
}

fn read_enumeration_list(node: &Node) -> Result<Vec<ValueEnumeration>> {
    let mut elist = Vec::new();
    for cnode in children(node) {
        let value = read_mandatory_attribute::<i64>(&cnode, "value")?;
        let label = read_mandatory_attribute::<String>(&cnode, "label")?;
        let max_value = read_attribute::<i64>(&cnode, "maxValue")?.unwrap_or(value);
        let description = read_attribute::<String>(&cnode, "shortDescription")?;
        elist.push(ValueEnumeration { value, label, max_value, description });
    }
    Ok(elist)
}

fn read_unit_set(node: &Node) -> Result<Vec<UnitType>> {
    let mut units = Vec::new();
    for cnode in children(node) {
        match cnode.tag_name().name() {
            "Unit" => {
                let power = read_attribute::<f64>(&cnode, "power")?.unwrap_or(1f64);
                let factor = read_attribute::<String>(&cnode, "factor")?.unwrap_or("1".to_owned());
                let description = read_attribute::<String>(&cnode, "description")?;
                let unit = cnode
                    .text()
                    .ok_or_else(|| get_parse_error("No unit present".to_owned(), &cnode))?;
                units.push(UnitType { unit: unit.trim().to_owned(), power, factor, description });
            }
            _ => log::warn!("ignoring unit set unknown property '{}'", cnode.tag_name().name()),
        }
    }
    Ok(units)
}
