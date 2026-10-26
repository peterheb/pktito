// The desktop's own time formatting (ui/app.js), run over times, rates and
// lengths, for the Android app's TimesTest to hold Times.kt to: prints JSON,
// { codes: [t, rate, timecode, rateLabel], stamps: [t, len, stamp(t, len),
// hms(t, len)], full: [t, stamp(t), hms(t)] }. Run again after a change to
// app.js's formatting:
//   node android/timecodes.js ui/app.js > android/app/src/test/resources/timecodes.json
const fs = require("fs");
const src = fs.readFileSync(process.argv[2], "utf8");
const grab = (re) => { const m = src.match(re); if (!m) throw new Error("missing " + re); return m[0]; };
const code = [
  grab(/const two = \(n\) => [^\n]*\n/),
  grab(/function hms\(s[^)]*\) \{[\s\S]*?\n\}\n/),
  grab(/function stamp\(s[^)]*\) \{[\s\S]*?\n\}\n/),
  grab(/function exactRate\(r\) \{[\s\S]*?\n\}\n/),
  grab(/function timecode\(t, rate\) \{[\s\S]*?\n\}\n/),
  grab(/const rateLabel = \(r\) => [^\n]*\n/),
].join("\n");
const f = new Function(code + "\nreturn { hms, stamp, exactRate, timecode, rateLabel };")();
const rates = [23.976, 24, 25, 29.97, 30, 50, 59.94, 60, 10, 12.5, 15, 23.98, 29.97003, 30.0001, 47.952, 48, 119.88, 120, 59.940059];
const times = [0, 0.001, 0.0417, 0.04170833, 0.5, 0.999, 1, 1.001, 9.99, 59.94, 59.96, 60, 60.06, 66.733, 599.4, 600, 600.6, 1799.4, 3599.9, 3600, 3600.0001, 35999.99, 36000, 86399.5, 360000.25];
let seed = 12345; const rnd = () => (seed = (seed * 1103515245 + 12345) % 2147483648) / 2147483648;
for (let i = 0; i < 400; i++) times.push(Math.round(rnd() * 7200 * 1e6) / 1e6);
const lens = [0, 59.99, 600, 640.505, 3599.99, 3600, 3600.5, 7200, 35999.99, 36000, 360000];
const out = { codes: [], stamps: [], full: [] };
for (const r of rates) for (const t of times) out.codes.push([t, r, f.timecode(t, r), f.rateLabel(r)]);
for (const len of lens) for (const t of times) out.stamps.push([t, len, f.stamp(t, len), f.hms(t, len)]);
for (const t of times) out.full.push([t, f.stamp(t), f.hms(t)]);
console.log(JSON.stringify(out));
