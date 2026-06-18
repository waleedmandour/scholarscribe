# ScholarScribe — Ethical Use Policy

This document is the authoritative statement of ScholarScribe's ethical commitments. It expands on the brief summary in the main README.

## 1. Purpose

ScholarScribe exists to help researchers who are writing their own manuscripts work with AI assistance:

- **transparently** (via disclosure statements),
- **privately** (via local LLMs, no cloud calls),
- and **authentically** (via style analysis that compares drafts to the author's own writing).

## 2. What we will not build

The following features are out of scope and will not be added to ScholarScribe, regardless of user demand:

1. **AI-detection evasion.** Any feature whose stated or implicit purpose is to lower a detector's score (Turnitin AI, GPTZero, Originality.ai, Copyleaks, etc.). This includes "marker targeting", "humanizer" pipelines, "AI-stealth" modes, and adversarial-perturbation engines.
2. **Detection-score feedback loops.** Running a detector in-app and showing "your AI score dropped from 92% to 14%" — even framed as informational — primarily serves evasion.
3. **Misrepresentation aids.** Features whose purpose is to obscure that AI was used at all, including fake "draft history" generators or "make this look like a real revision trail" tools.
4. **Citation fabrication.** Generating references that don't exist. The chat system prompt explicitly forbids this and instructs the model to ask the user for sources.

## 3. Why these lines

The author of this project began from a repo whose stated purpose included evading AI-plagiarism detectors. After reflection, that direction was rejected for three reasons:

1. **Net harm to research integrity.** Evasion tools undermine the social contract that makes peer review work. Even when used by honest researchers, they make the whole detection ecosystem less trustworthy — which hurts honest researchers most.
2. **Disproportionate impact on non-native English writers.** Independent evaluations (Liang et al., 2023; Weber-Wulff et al., 2023) show AI detectors have high false-positive rates on writing by non-native English authors. An arms race between evaders and detectors makes this worse, not better. The right response is institutional — push back on detector use, not build better evasion.
3. **Doesn't actually solve the user's problem.** If a researcher's real concern is "I used AI and I'm worried about being accused" — the answer is *disclosure*, not evasion. Disclosure sidesteps the detector entirely and is required by every major venue's policy. ScholarScribe makes disclosure easy.

## 4. Features that fit

The following are in scope and welcome as contributions:

- Local LLM running (any open model Ollama supports).
- Writing aids: paraphrasing the author's own sentences, suggesting alternative phrasings, critique, outlining.
- Style analysis comparing a draft to the author's own prior writing.
- Disclosure-statement generation for any venue with a public AI-use policy.
- Educational content about AI detectors and their limitations.
- Privacy improvements: offline-first architecture, no telemetry, auditable network calls.
- Accessibility and i18n.

## 5. Handling pull requests

Pull requests will be reviewed against this policy. PRs that add evasion features will be closed without merge, with a pointer to this document. This is not censorship — the author is happy to fork the project under a different name if your use case genuinely requires evasion. But it won't ship under the ScholarScribe name.

## 6. Changes to this policy

This policy may evolve, but the core commitments (no evasion, no telemetry, local-first) are treated as load-bearing. Any change to them will be discussed in a public GitHub issue before merge.

## 7. Further reading

- Liang, W., et al. *GPT detectors are biased against non-native English writers.* Patterns, 2023. <https://doi.org/10.1016/j.patter.2023.100779>
- Weber-Wulff, D., et al. *Testing of detection signals for AI-generated text.* Patterns, 2023.
- ICMJE Recommendations, section on AI-assisted technology. <https://www.icmje.org/recommendations/>
- Nature Editorial Policy on AI. <https://www.nature.com/editorial-policies/ai>
