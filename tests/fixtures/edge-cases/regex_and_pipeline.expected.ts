const matches = __binop__(text.match(/a|b/), "|>", Array.from);
const filtered = __binop__(/foo|bar/.test(x), "|>", Boolean);
const result = __binop__(__binop__(data, "|>", transform), "|>", filterWith(/pattern|alt/));
