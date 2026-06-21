<script lang="ts">
  // Static educational content. No backend calls.
</script>

<h1>Detector Literacy</h1>
<p class="lead">
  How AI-detection tools (Turnitin's AI score, GPTZero, Originality.ai, etc.) actually work — what they measure,
  where they fail, and what that means for you as a researcher. This module is educational only; it does not help
  you evade detection.
</p>

<div class="card">
  <div class="card-title">The two big ideas: perplexity and burstiness</div>
  <p>
    Most AI detectors work by scoring text on two derived measures and comparing the score to a threshold:
  </p>
  <ul style="line-height: 1.7;">
    <li>
      <strong>Perplexity</strong> — roughly, "how surprised was the language model by each next word?" Human writing
      tends to be less predictable than LLM output, so it scores <em>higher</em> perplexity. Low perplexity is one
      signal that text may be AI-generated.
    </li>
    <li>
      <strong>Burstiness</strong> — the variation in sentence length and structure across a passage. Human writing
      mixes short and long sentences. LLM output tends to be more uniform, so low burstiness is a flag.
    </li>
  </ul>
  <p class="muted" style="font-size: 13px;">
    A third family of detectors looks for stylistic "markers" (overused transitions, formulaic paragraph shapes,
    specific lexical choices). These marker-based detectors are the least reliable but increasingly common.
  </p>
</div>

<div class="card">
  <div class="card-title">Where detectors fail — and who pays the price</div>
  <p>Independent evaluations (Liang et al., 2023; Weber-Wulff et al., 2023; Laban et al., 2024) repeatedly find:</p>
  <ul style="line-height: 1.7;">
    <li>
      <strong>False-positive bias against non-native English writers.</strong> Text by authors with lower English
      proficiency tends to score as "more AI-like" — shorter sentences, simpler connectors, lower perplexity — even
      when entirely human-written. This is a documented equity problem, not a marginal one.
    </li>
    <li>
      <strong>High false-positive rate on short passages.</strong> Detectors are unreliable below ~300 words. A single
      suspicious sentence is not evidence of anything.
    </li>
    <li>
      <strong>Sensitivity to editing.</strong> A human-edited AI draft may score as "human"; a human draft lightly
      polished by Grammarly may score as "AI". The boundary is noisy.
    </li>
    <li>
      <strong>Adversarial fragility.</strong> Light paraphrasing, translation round-trips, or stylistic prompts can
      push AI text past detectors. This is why <em>detection scores are not a reliable integrity signal on their own</em> —
      which is also why building tools to "beat" them is a fool's errand: the underlying signal was never trustworthy
      enough to defend.
    </li>
  </ul>
</div>

<div class="card">
  <div class="card-title">What this means for you</div>
  <ul style="line-height: 1.7;">
    <li>
      <strong>If you wrote the draft yourself</strong>, you do not need to "beat" a detector — you may legitimately
      be flagged by one. The correct response is to push back institutionally: ask for the detector's false-positive
      rate, request human review, and cite the literature above.
    </li>
    <li>
      <strong>If you used AI assistance</strong>, the correct response is to <em>disclose it</em> (see the Disclosure
      tab). Disclosure sidesteps the detector entirely and aligns with every major journal's policy.
    </li>
    <li>
      <strong>If you are an instructor or reviewer</strong>, never use an AI-detection score as the sole basis for an
      integrity finding. Combine with interviews, version history, and other evidence. Several universities (Vanderbilt,
      Pitt, Texas-Austin) have stopped using Turnitin's AI score for exactly this reason.
    </li>
  </ul>
</div>

<div class="card">
  <div class="card-title">Further reading</div>
  <ul style="line-height: 1.7;">
    <li>Liang, W., Yuksekgonul, M., Mao, Y., Wu, E., & Zou, J. (2023). <em>GPT detectors are biased against non-native English writers.</em> Patterns, 4(7), 100779. <a href="https://doi.org/10.1016/j.patter.2023.100779" target="_blank" rel="noopener">DOI: 10.1016/j.patter.2023.100779</a></li>
    <li>Weber-Wulff, D., Anohina-Naumeca, A., Bjelobaba, S., et al. (2023). <em>Testing of detection tools for AI-generated text.</em> International Journal for Educational Integrity, 19, 26. <a href="https://doi.org/10.1007/s40979-023-00146-z" target="_blank" rel="noopener">DOI: 10.1007/s40979-023-00146-z</a></li>
    <li>International Committee of Medical Journal Editors. (2024). <em>Recommendations for the conduct, reporting, editing, and publication of scholarly work in medical journals.</em> <a href="https://www.icmje.org/recommendations/" target="_blank" rel="noopener">icmje.org/recommendations</a></li>
    <li><em>Nature</em> Editorial Policy on AI. (2024). <a href="https://www.nature.com/editorial-policies/ai" target="_blank" rel="noopener">nature.com/editorial-policies/ai</a></li>
    <li>University guidance: Vanderbilt, Pitt, Texas-Austin statements on AI-detector reliability (searchable online).</li>
  </ul>
</div>

<div class="callout info">
  <strong>Why this tab exists.</strong>
  Understanding detectors is part of research literacy in 2026. But "understanding" and "evading" are different
  activities. ScholarScribe will not add features whose purpose is to lower detector scores — that path leads to an
  arms race that harms honest researchers (especially non-native English writers) without solving the real problem:
  transparent communication about AI assistance.
</div>
