# humanizeai — ARCHIVED

> **This project has been superseded. The active project is now [ScholarScribe](https://github.com/waleedmandour/scholarscribe).**

## Why this repo was archived

The original `humanizeai` project was built to help researchers reduce AI-detection scores on Turnitin and similar tools. After reflection, the maintainer decided not to continue in that direction. Reducing AI-detection scores — even when used by honest researchers — primarily serves to deceive reviewers and instructors about whether AI was used. That:

- undermines the integrity of peer review,
- disproportionately harms honest researchers (especially non-native English writers, who are most often falsely accused of AI use and would be most harmed by an arms race between evaders and detectors), and
- doesn't actually solve the underlying problem researchers have — which is best addressed by transparent disclosure of AI assistance, not by hiding it.

## What replaced it

**[ScholarScribe](https://github.com/waleedmandour/scholarscribe)** is a privacy-first, local-LLM writing companion for researchers. It keeps everything valuable from the original project's direction — 100% free, no paid APIs, runs entirely on the user's device, supports open LLMs like Gemma and Qwen — and drops the one part that couldn't be justified: detection evasion.

ScholarScribe's four modules:

1. **Models** — Install and run open LLMs locally via Ollama.
2. **Style Analysis** — Compare a draft to a sample of *your own* prior writing (consistency check, not evasion).
3. **Disclosure Assistant** — Generate venue-compliant AI-use disclosure statements for ICMJE, Nature, IEEE, Elsevier, ACL.
4. **Detector Literacy** — Plain-English explainer of how AI detectors work and where they fail, with peer-reviewed citations.

No telemetry. No cloud calls. No third-party APIs. No detection-evasion features. See [ScholarScribe's ethical-use policy](https://github.com/waleedmandour/scholarscribe/blob/main/docs/ETHICS.md) for the full commitments.

## If you arrived here looking for an evasion tool

That tool no longer exists in any form maintained by this author. If you are a researcher worried about being falsely accused of AI use, the recommended path is:

- **Disclose** any AI assistance you did use (ScholarScribe's Disclosure Assistant helps with this).
- **Push back institutionally** if falsely accused — ask for the detector's false-positive rate, request human review, and cite the literature (Liang et al. 2023, Weber-Wulff et al. 2023) on detector unreliability, especially for non-native English writers. Several universities (Vanderbilt, Pitt, Texas-Austin) have stopped using Turnitin's AI score for exactly this reason.

The right fix for the detector problem is better detectors and better institutional policies — not better evasion.

## License

The historical code in this archived repository remains under its original license. New work happens at [ScholarScribe](https://github.com/waleedmandour/scholarscribe) under the MIT license.
