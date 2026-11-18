// Read direct exported factory declarations bound to named imports in the same file.
module.exports = function factories(binding, groups) {
  const entries = [];
  for (const group of groups.values()) for (const {path, tree} of group.trees) {
    const imports = new Map();
    for (const statement of tree.rootNode.namedChildren) {
      if (statement.type === 'import_statement') {
        const source = statement.childForFieldName('source')?.firstNamedChild?.text;
        if (source !== binding.module || statement.children.some(n => n.type === 'type')) continue;
        for (const spec of statement.descendantsOfType('import_specifier')) {
          if (spec.children.some(n => n.type === 'type')) continue;
          const name = spec.childForFieldName('name')?.text;
          const local = spec.childForFieldName('alias')?.text ?? name;
          if (name === binding.export) imports.set(local, name);
        }
      }
    }
    for (const statement of tree.rootNode.namedChildren) {
      const decl = statement.type === 'export_statement' && statement.childForFieldName('declaration');
      if (decl?.type !== 'lexical_declaration' || decl.firstChild.text !== 'const') continue;
      for (const variable of decl.namedChildren.filter(n => n.type === 'variable_declarator')) {
        const name = variable.childForFieldName('name'), call = variable.childForFieldName('value');
        if (name?.type !== 'identifier' || call?.type !== 'call_expression') continue;
        const fn = call.childForFieldName('function');
        const direct = binding.member === undefined && fn?.type === 'identifier' && imports.has(fn.text);
        const member = fn?.type === 'member_expression' && imports.has(fn.childForFieldName('object')?.text)
          && fn.childForFieldName('property')?.text === binding.member;
        if (direct || member) entries.push({path, name:name.text});
      }
    }
  }
  return {[binding.fact]:entries};
};
