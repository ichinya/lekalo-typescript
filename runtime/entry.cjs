const {run} = require('../parser/runtime/tree-sitter/index.cjs');
const config = require('./grammar.json');
const httpFacts = require('./http.cjs');
run(config, __dirname, httpFacts).catch(() => { process.exitCode = 2; });
