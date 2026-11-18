// A bounded reader for import-bound routers with direct, top-level method calls.
module.exports = function httpFacts(pack, check, groups, packs) {
  const required = pack.rules.requires;
  if (required.length !== 1) throw Error('unsupported-facts');
  if (required[0] !== 'http.routes/v1') {
    const binding = pack.rules.bindings?.['typescript.exported-factories/v1'];
    if (!binding || binding.capability !== required[0]) throw Error('unsupported-facts');
    return require('./factories.cjs')(binding, groups);
  }
  const readers = packs.flatMap(p => p.rules.bindings?.['typescript.static-router/v1'] ?? []);
  if (!readers.length || readers.some(r => JSON.stringify(r) !== JSON.stringify(readers[0]))) throw Error('router-binding');
  const binding = readers[0], routes = [];
  for (const group of groups.values()) for (const {path, tree} of group.trees) {
    if (!binding.files.includes(path)) continue;
    const constructors = new Set(), routers = new Set();
    for (let node of tree.rootNode.namedChildren) {
      if (node.type === 'comment') continue;
      if (node.type === 'import_statement') {
        const source = node.childForFieldName('source');
        if (source?.namedChildren[0]?.text === binding.module) {
          for (const item of node.descendantsOfType('import_specifier')) {
            if (item.childForFieldName('name')?.text === binding.export) {
              constructors.add((item.childForFieldName('alias') ?? item.childForFieldName('name')).text);
            }
          }
        }
        continue;
      }
      if (node.type === 'export_statement') {
        const declaration = node.childForFieldName('declaration');
        if (!declaration && routers.has(node.childForFieldName('value')?.text)) continue;
        if (!declaration) throw Error('unknown-export');
        node = declaration;
      }
      if (node.type === 'lexical_declaration' && node.children[0].text === 'const') {
        for (const variable of node.namedChildren.filter(n => n.type === 'variable_declarator')) {
          const value = variable.childForFieldName('value'), name = variable.childForFieldName('name');
          if (name?.type !== 'identifier' || value?.type !== 'new_expression'
              || !constructors.has(value.childForFieldName('constructor')?.text)
              || value.childForFieldName('arguments')?.namedChildCount !== 0) throw Error('unknown-constructor');
          routers.add(name.text);
        }
        continue;
      }
      const call = node.type === 'expression_statement' && node.firstNamedChild;
      const member = call?.type === 'call_expression' && call.childForFieldName('function');
      if (!member || member.type !== 'member_expression' || !routers.has(member.childForFieldName('object')?.text)) throw Error('unknown-statement');
      const method = binding.methods[member.childForFieldName('property')?.text];
      const args = call.childForFieldName('arguments')?.namedChildren;
      const literal = args?.[0];
      if (typeof method !== 'string' || !args || args.length < 2 || literal?.type !== 'string'
          || literal.namedChildCount !== 1 || literal.firstNamedChild.type !== 'string_fragment') throw Error('unknown-route');
      routes.push({method, path: literal.firstNamedChild.text});
    }
  }
  routes.sort((a,b) => a.path < b.path ? -1 : a.path > b.path ? 1 : a.method < b.method ? -1 : a.method > b.method ? 1 : 0);
  return {routes};
};
