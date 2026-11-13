#!/bin/sh
# Regenerates tests/golden/expected.txt from tests/golden/cases.txt using Yamcs (needs java and maven).
# The Yamcs version is set in pom.xml.
set -e
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../.." && pwd)
cd "$here"
mvn -q dependency:build-classpath -Dmdep.outputFile=target/cp.txt
java -cp "$(cat target/cp.txt)" src/main/java/Golden.java "$root/test-xtce-files" \
    "$root/tests/golden/cases.txt" "$root/tests/golden/expected.txt"
