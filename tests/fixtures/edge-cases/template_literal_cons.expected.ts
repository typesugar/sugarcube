const list = `Items: ${__binop__(1, "::", __binop__(2, "::", []))}`;
const nested = `outer ${`inner ${__binop__(x, "::", xs)}`}`;
const multi = `first: ${__binop__(a, "::", as)} second: ${__binop__(b, "::", bs)}`;
const noInterp = `some :: text :: here`;
