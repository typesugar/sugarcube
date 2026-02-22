const msg = `Result: ${data |> transform |> format}`;
const nested = `outer ${`inner ${x |> f}`}`;
const multi = `a ${a |> fa} b ${b |> fb} c`;
const noInterp = `plain |> text should not change`;
