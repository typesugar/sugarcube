const msg = `Result: ${__binop__(__binop__(data, "|>", transform), "|>", format)}`;
const nested = `outer ${`inner ${__binop__(x, "|>", f)}`}`;
const multi = `a ${__binop__(a, "|>", fa)} b ${__binop__(b, "|>", fb)} c`;
const noInterp = `plain |> text should not change`;
