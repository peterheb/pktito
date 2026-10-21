-- Installer downloads, counted a day at a time (UTC) for each release
-- file: the counts and nothing else (worker.js, count).
CREATE TABLE downloads (
  day     TEXT    NOT NULL,  -- YYYY-MM-DD
  version TEXT    NOT NULL,  -- 0.1.0
  file    TEXT    NOT NULL,  -- pktito_0.1.0_aarch64.dmg
  n       INTEGER NOT NULL,
  PRIMARY KEY (day, version, file)
);
