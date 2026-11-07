// The page's cancel word for this crate's WebAssembly test and bench binaries, never the web core
// (kernel.md section 3.12 item 9): one BigInt64Array over shared memory that holds the request 7,
// for test 166 and the web row of the polls' cost.
Module['partsmithCancelWord'] = new BigInt64Array(new SharedArrayBuffer(8))
Module['partsmithCancelWord'][0] = 7n
