# xtce-rs

Rust library for working with [XTCE](https://www.omg.org/spec/XTCE/) mission databases: it loads XTCE files,
decodes telemetry (TM) packets and encodes telecommands (TC).

The processing semantics follow [Yamcs](https://yamcs.org) (5.14) as closely as possible: the same value types,
rounding, validity and alarm rules, absolute time conventions, etc. The compatibility is checked by golden tests
comparing the xtce-rs results with Yamcs (see [Testing](#testing)).

## Usage

```rust
use std::{collections::HashMap, path::Path};
use xtce_rs::{parser, proc::TmProcessor, tc::encode_command, value::Value};

let mdb = parser::parse_files(&[Path::new("test-xtce-files/simulator.xml")])?;

// TM: the processor keeps the state (previous values, algorithm inputs) between packets
let root = mdb.search_container("/YSS/SIMULATOR/FlightData").unwrap();
let mut tmp = TmProcessor::new(&mdb)?;
let params = tmp.process(&packet, root, generation_time, acquisition_time)?;
for pv in params.iter() {
    println!("{:?}", pv.dbg(&mdb));
}

// TC: the argument values can be given as strings (parsed according to the argument type) or typed values
let cmd = mdb.search_command("/YSS/SIMULATOR/SWITCH_VOLTAGE_ON").unwrap();
let args = HashMap::from([("voltage_num".to_owned(), Value::StringValue(Box::new("1".into())))]);
let encoded = encode_command(&mdb, cmd, &args)?;
println!("{}", hex::encode(&encoded.binary));
```

`proc::containers::process(&mdb, packet, root)` decodes a single packet without state.

`TmProcessor::process_with` takes `ProcessOptions`: with `details` it also returns, for each value, its bit offset
and size in the packet and the container defining the entry, the extracted containers (`TmResult::packet_container`
gives the packet name like the Yamcs packet viewer) and the applied alarm ranges; `algorithms: false` skips the
algorithms.

`parser::parse_strs` loads XTCE documents given as strings (no file system needed); `parse_strs_with(…, &ParseOptions { commands: false })` skips the `CommandMetaData`. The library builds for
`wasm32-unknown-unknown`.

`xtce_rs::format` prints values and calibrators like Yamcs (`Value.toString()`), including Java's
`Double.toString`/`Float.toString` (`1.0E10`); checked against the JDK with `tools/javafmt`.

The times are *instants*: milliseconds since 1970-01-01 TAI including the leap seconds, like the Yamcs internal
time (see `xtce_rs::time` for conversions).

To see what parts of an XTCE file are not supported: `RUST_LOG=warn cargo run --example load -- file.xml`.

## Feature matrix

✅ supported, ⚠️ partially supported, ❌ not supported

### Mission database

| Feature | Status | Notes |
|---|---|---|
| SpaceSystem hierarchy, relative and absolute references | ✅ | multiple files can be loaded together (`parse_files`) |
| Integer, Float, Boolean, Enumerated, String, Binary, AbsoluteTime parameter types | ✅ | |
| Aggregate and Array parameter types, references to members (`p.m`, `p[1]`) | ✅ | |
| RelativeTime parameter types | ❌ | |
| Type inheritance (`baseType`) | ✅ | |
| Aliases (AliasSet) | ✅ | |
| ParameterProperties `dataSource` | ✅ | |
| Type `initialValue` | ✅ | used for the arguments and, like Yamcs, as initial value of the TM parameters |
| Parameter `initialValue` | ❌ | only the type initial value is used |
| MessageSet, StreamSet, ServiceSet | ❌ | ignored |

### Data encodings

| Feature | Status | Notes |
|---|---|---|
| Integer: unsigned, twos complement, sign magnitude, ones complement, 1-64 bits, both byte orders | ✅ | |
| Integer: BCD, packed BCD | ❌ | |
| Float: IEEE754 16/32/64 bits, MIL-STD-1750A 32/48 bits | ✅ | |
| String: fixed size, termination char, leading size, `Variable` (XTCE 1.2), dynamic size | ✅ | US-ASCII, ISO-8859-1, UTF-8, UTF-16 (BE/LE) |
| Integer and float encoded as strings | ✅ | |
| Binary: fixed size, dynamic size | ✅ | |
| Binary: Yamcs `LeadingSizeBinaryDecoder/Encoder` algorithms | ✅ | implemented natively |
| Absolute time: Yamcs `TimeBinaryDecoder` (CUC, without `tcoService`) | ✅ | implemented natively, as Yamcs (the `epoch` option is ignored) |
| Other custom (Java) transform algorithms | ❌ | |
| Boolean encoding | ✅ | |
| Calibrators: polynomial, spline, MathOperation | ✅ | |
| Context calibrators | ✅ | |
| Java expression calibrators | ❌ | |

### Telemetry decoding

| Feature | Status | Notes |
|---|---|---|
| SequenceContainer inheritance with RestrictionCriteria (Comparison, ComparisonList, BooleanExpression) | ✅ | |
| ParameterRefEntry, ContainerRefEntry, ArrayParameterRefEntry, IndirectParameterRefEntry | ✅ | |
| LocationInContainerInBits (containerStart, previousEntry) | ✅ | |
| LocationInContainerInBits containerEnd | ❌ | |
| RepeatEntry (fixed and dynamic count, offset) | ✅ | Yamcs ignores the offset |
| IncludeCondition | ✅ | |
| Dynamic sizes and locations (DynamicValue with LinearAdjustment) | ✅ | |
| Parameter instances (`instance="-1"`) in dynamic values and algorithms | ✅ | like Yamcs, the instance is ignored in match criteria |
| Yamcs value types (SINT32, UINT64, FLOAT...) and narrowing | ✅ | `DataType::eng_value_type`, `DataEncoding::raw_value_type` |
| Absolute times: epochs (TAI, J2000, UNIX, GPS, date), OffsetFrom, scale and offset | ✅ | leap seconds as in Yamcs |
| Valid ranges (raw or calibrated) | ✅ | the value is marked invalid |
| Alarms: static ranges, enumeration alarms, context alarms, default alarm level | ✅ | `MonitoringResult` and `RangeCondition` as in Yamcs |
| Time based or change based alarms, alarm `minViolations` | ❌ | |
| MathAlgorithm (with triggers, chained algorithms) | ✅ | |
| CustomAlgorithm (Java, JavaScript, Python) | ❌ | ignored with a warning |

### Telecommand encoding

| Feature | Status | Notes |
|---|---|---|
| MetaCommand inheritance with ArgumentAssignment, abstract commands | ✅ | |
| Argument types of all the kinds above, including aggregates and arrays | ✅ | values given as typed values or strings (JSON for aggregates and arrays, like Yamcs) |
| Argument and type initial values | ✅ | |
| Validation: valid ranges, integer sizes, string and binary size ranges, enumeration labels | ✅ | |
| ArgumentRefEntry, FixedValueEntry, ParameterRefEntry | ✅ | the parameter values are given in `EncodeOptions` |
| LocationInContainerInBits (fixed) | ✅ | |
| Command container BinaryEncoding size (padding) | ✅ | |
| Calibrated arguments | ✅ | like Yamcs, the calibrator of an argument type converts from engineering to raw |
| ContainerRefEntry to the CommandContainerSet, RepeatEntry | ✅ | not supported by Yamcs (see `tc-extensions.xml`) |
| Absolute time arguments relative to a parameter (OffsetFrom) | ❌ | |
| Dynamic values, IncludeCondition in command containers | ❌ | |
| Transmission constraints, verifiers, significance | ❌ | parsed but not used |

## Differences from Yamcs

xtce-rs reports an error, or encodes according to XTCE, in some cases where Yamcs (5.14) gives a different
result. Most of these are Yamcs bugs or limitations:

**TC encoding**
- Yamcs encodes negative values for sign magnitude integer encodings incorrectly. `DataEncodingUtils.getRawIntegerValue` masks the raw value to the encoding size, so the encoder never sees a negative value and writes the twos complement bits (-3 on 8 bits gives `0xFD` instead of `0x83`). The `SIGN_MAGNITUDE` branch of `DataEncodingEncoder.encodeRawInteger` is also wrong: it never sets the sign bit and computes `v &= (1 << (64 - sizeInBits))` with an `int` shift. Ones complement is not implemented there at all (`UnsupportedOperationException`).
- Yamcs silently truncates calibrated raw values, and absolute times, that do not fit in the encoding. For example a 32 bits time in milliseconds since GPS wraps after 49 days. xtce-rs reports an error.
- The Yamcs XTCE loader skips the `CommandContainerSet` and ignores `RepeatEntry` and `ContainerRefEntry` in the command containers.
- The Yamcs XTCE loader does not read `SizeRangeInBytes` for binary arguments, so their size is not checked.
- Yamcs ignores the arguments that are not defined by the command. xtce-rs reports an error.

**TM decoding**
- The Yamcs XTCE loader ignores the `Offset` of a `RepeatEntry` (`XtceStaxReader.readRepeatEntry` reads only the `Count`). xtce-rs applies it.
- In `MatchCriteriaEvaluatorFactory.AbstractComparisonEvaluator.evaluate`, the int/float mixed comparisons test `lValue.evaluator` against both evaluators (`lValue.evaluator == intEvaluator && lValue.evaluator == floatEvaluator`). The condition is always false, so comparing an integer parameter with a float value (or the reverse) is never done; it only logs an error.

- XTCE allows the `Epoch` of an absolute time to be a date (`xs:date`, e.g. `1970-01-01`) but `TimeEncoding.parse` used by Yamcs accepts only date-times: the parameters of such types get no engineering value and are marked invalid.

**Matched on purpose**

xtce-rs follows these Yamcs behaviours, which may be surprising:
- The `instance` of the parameter references in match criteria (restriction criteria, include conditions, context matches) is ignored: the current value is used.
- The calibrators of the argument types are applied in the engineering → raw direction.
- The parameters whose types have an initial value start with that value in the cache, for example the mission constants used in include conditions.

## Testing

```sh
cargo test
```

- `tests/golden.rs` compares the TM decoding and the TC encoding with the results produced by Yamcs.
  - The cases are in `tests/golden/cases.txt`.
  - The expected results in `tests/golden/expected.txt` are regenerated with `tools/yamcs-golden/generate.sh`, which needs Java 17+ and Maven and downloads Yamcs from Maven Central.
  - The algorithms and the alarms are not covered, because the Yamcs container extractor does not run them; they are tested in `tests/phase2.rs`.
- `tests/robustness.rs` checks with random and mutated packets, arguments and XTCE files that nothing panics.
  - The number of iterations can be increased with `XTCE_FUZZ_ITERATIONS`.
- The `fuzz` directory contains [cargo-fuzz](https://github.com/rust-fuzz/cargo-fuzz) targets for coverage guided fuzzing (needs a nightly toolchain): `cargo +nightly fuzz run tm_decode` (or `tc_encode`, `xtce_parse`).
