Checks `xtce_rs::format::java_double` / `java_float` against Java's `Double.toString` / `Float.toString`
(JDK 19 or later) on random values:

```sh
java tools/javafmt/Gen.java > /tmp/cases.txt
cargo run --release --example javafmt_check /tmp/cases.txt
```
