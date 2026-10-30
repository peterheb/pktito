# The speech test set

Twenty-eight clips, 96 minutes in all, of the speech people bring to pktito
besides audiobooks: arguments in court, a news conference, interviews, logs
recorded alone, a 1969 spacecraft cabin, and scripts read aloud in English
and Spanish. Every recording is the U.S. government's, so in the public
domain, and 25 of the clips were recorded in 2025 or 2026, after the models
were trained, so the models haven't heard them. The set is for checking
transcripts on more than AliceBench's one reader, and for making and
scoring a model that tidies them: fillers, punctuation, paragraphs,
numbers, names.

| Style      | Clips | Minutes | From                                                                                                                     |
| ---------- | ----- | ------- | ------------------------------------------------------------------------------------------------------------------------ |
| court-qa   | 6     | 21      | the Supreme Court's October 2026 Term; the Ninth Circuit                                                                 |
| press-qa   | 2     | 7       | NASA's Crew-13 postlaunch news conference, some questions by phone                                                       |
| interview  | 13    | 45      | NASA's Houston We Have a Podcast; CDC's Emerging Infectious Diseases and lead-poisoning podcasts; the NOAA Ocean Podcast |
| monologue  | 2     | 7       | CHAPEA crew members' audio logs (NASA)                                                                                   |
| historical | 2     | 6       | Apollo 11's onboard tapes, with hiss and dropouts                                                                        |
| scripted   | 3     | 10      | CDC: MMWR's weekly summary, and two scripts in Spanish                                                                   |

The audio isn't kept here. `fetch.py` downloads each recording from where
it's published (1.06 GB), checks it against the SHA-256 in `manifest.json`,
and cuts the clips as 16 kHz mono FLAC, so that every machine reads the
same samples, AliceBench's reason for FLAC:

```
python3 testset/fetch.py                    # into testset/clips, which git leaves out
python3 testset/fetch.py --only court-qa interview_cdc-eid-cre_1
```

It needs ffmpeg and nothing else, and says for each clip whether its
samples are the ones the set was made from; another version of ffmpeg may
decode a source slightly differently. Each clip begins and ends in a pause,
found by Silero VAD, and the clips leave out podcasts' openings and
closings, where the music is.

- `manifest.json`, and `manifest.csv` as a table: each clip's source, its
  start and end in the source, who is speaking and how it was recorded, its
  subject and language, how much of it is speech, the rights, and a SHA-256
  of its samples.
- `transcripts/`: pktito 0.9.3's transcripts of every clip, by the .deb's
  pktito-cli, as `.txt` and as `.json` with each word's times.
  `parakeet-tdt-0.6b-v2` has the English model's, including what it makes
  of the two Spanish clips; `parakeet-tdt-0.6b-v3` has the multilingual
  model's of those two. They are what a tidying model starts from, made
  with `pktito-cli -f json` and `-f txt` from the fetched clips.
- `refs/`: the published transcript of each whole recording, for the 19 of
  the 24 that have one (the Court's and CDC's PDFs, NASA's and NOAA's
  episode pages), as text. They aren't aligned to the clips. The podcasts'
  are lightly edited, with some fillers taken out; the Court's are close to
  verbatim, line numbers and all.
- `tools/`: how the set was made. `sources.json` lists the 24 recordings.
  `download.py` fetches them; `select_clips.py` chooses each clip's start
  and end, into `selection.json`, with Silero VAD (`vadlib.py`, which needs
  numpy, onnxruntime, and `silero_vad.onnx` from snakers4/silero-vad);
  `cut.py` cuts the clips and writes the manifest; `refs_to_text.py` turns
  the published transcripts into text. A recording added to `sources.json`
  goes through the same steps.

## Rights

Works of the U.S. government aren't under copyright (17 U.S.C. § 105). That
covers the courts', NASA's, CDC's and NOAA's recordings, and the Apollo
tapes, which the Internet Archive marks with the Public Domain Mark. NASA
asks that recordings of people who can be identified not be used
commercially without their consent, so none of these voices should go into
marketing. Podcasts' music can be licensed, one more reason the clips leave
out openings and closings, and the reason this folder keeps a manifest and
a script rather than the audio.

## Not included

- The Federal Reserve's FOMC press conferences, dense with numbers: the Fed
  shows them in an embedded video player, so no script here fetches them.
- DVIDS's military audio: the site turns scripts away, and its API needs a
  key.
- The JFK Library's press conferences, behind Cloudflare.
- Common Voice's Spontaneous Speech (CC0, everyday talk), which needs an
  account and may not be shared on.
- Dictation (emails, notes, lists): nothing in the public domain sounds
  like it.
