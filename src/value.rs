
struct EnumeratedValue {

}
enum Value {
    Int32(i32),
    Uint32(u32),
    Int64(i64),
    Uint64(u64),
    Float(f32),
    Double(f64),
    StringValue(String),
    Enumerated(EnumeratedValue),
    Binary()
}