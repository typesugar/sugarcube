const matches = text.match(/a|b/) |> Array.from;
const filtered = /foo|bar/.test(x) |> Boolean;
const result = data |> transform |> filterWith(/pattern|alt/);
