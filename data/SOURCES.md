# Offline word lists

Downloaded on 2026-11-10. The processed word lists are retained byte-for-byte in
`dictionaries.b64` (a Base64 encoded ZIP) for reproducible offline builds. The local development workspace
also retains raw downloads; upstream locations are listed below.

* English: CMU Pronouncing Dictionary, https://github.com/cmusphinx/cmudict
  https://raw.githubusercontent.com/cmusphinx/cmudict/master/cmudict.dict
  License: Carnegie Mellon permissive/BSD-style; full terms in LICENSE-ENGLISH.
  Modified: extracted lowercase alphabetical headwords of 1–32 letters,
  removed pronunciation variants, sorted and deduplicated (117,493 words).
  Short words are retained to avoid changing valid English during live correction.
* Thai: PyThaiNLP, https://github.com/PyThaiNLP/pythainlp
  https://raw.githubusercontent.com/PyThaiNLP/pythainlp/main/pythainlp/corpus/words_th.txt
  This word list is CC0-1.0, as specified in upstream corpus_license.md
  (included as LICENSE-THAI.md). The upstream code license is also included
  as LICENSE-PYTHAINLP for reference.
  Modified: kept 2–96 character Thai-only entries, sorted and deduplicated,
  added a few everyday words (60,960 words).

Run `node tools/prepare-dictionaries.mjs` to recreate the packed dictionaries
from raw files downloaded from the locations above. `cargo build` automatically
extracts the processed archive into its build output; the running application
uses the same read-only embedded word lists. Python/Node is not needed to run it.
The English list contains names and uncommon words, so dictionary membership
does not imply grammatical correctness. No statistical language model is used.
