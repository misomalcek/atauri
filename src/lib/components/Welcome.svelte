<script>
  let { onopen } = $props();

  // Generate star field positions (deterministic from seed-like approach)
  const stars = Array.from({ length: 200 }, (_, i) => ({
    x: ((i * 7919 + 104729) % 10000) / 100,
    y: ((i * 6271 + 83557) % 10000) / 100,
    size: ((i * 3571) % 3) + 1,
    opacity: (((i * 4219) % 60) + 20) / 100,
    twinkle: ((i * 2731) % 5) + 3,
  }));
</script>

<div class="welcome">
  <!-- Star field -->
  <div class="starfield">
    {#each stars as star}
      <div
        class="star"
        style="
          left: {star.x}%;
          top: {star.y}%;
          width: {star.size}px;
          height: {star.size}px;
          opacity: {star.opacity};
          animation-duration: {star.twinkle}s;
        "
      ></div>
    {/each}
  </div>

  <!-- Orbital system -->
  <div class="orbit-system">
    <!-- SVG orbital paths -->
    <svg class="orbits" viewBox="-200 -200 400 400" xmlns="http://www.w3.org/2000/svg">
      <ellipse cx="0" cy="0" rx="100" ry="60" class="orbit-path orbit-1" />
      <ellipse cx="0" cy="0" rx="120" ry="75" class="orbit-path orbit-2" />
      <ellipse cx="0" cy="0" rx="85" ry="110" class="orbit-path orbit-3" />
    </svg>

    <!-- Central portal -->
    <div class="portal">
      <div class="portal-core"></div>
      <div class="portal-ring"></div>
      <div class="portal-ring ring-2"></div>
    </div>

    <!-- Orbiting stars -->
    <div class="star-orbit orbit-alpha">
      <div class="orbiting-star alpha">
        <span class="star-label">Local AI</span>
      </div>
    </div>

    <div class="star-orbit orbit-beta">
      <div class="orbiting-star beta">
        <span class="star-label">Cloud AI</span>
      </div>
    </div>

    <div class="star-orbit orbit-proxima">
      <div class="orbiting-star proxima">
        <span class="star-label">Emergence</span>
      </div>
    </div>
  </div>

  <!-- Content overlay -->
  <div class="content">
    <h1 class="title">Tauri Code</h1>
    <p class="subtitle">AI-first development interface</p>
    <p class="tagline">context weaving meets code</p>

    <button class="open-btn" onclick={onopen}>
      <span class="btn-glow"></span>
      Open Project
    </button>

    <div class="trinity">
      <span class="tri-local">Local</span>
      <span class="tri-op">+</span>
      <span class="tri-cloud">Cloud</span>
      <span class="tri-op">=</span>
      <span class="tri-emerge">Emergence</span>
    </div>
  </div>
</div>

<style>
  .welcome {
    position: relative;
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
    background: radial-gradient(ellipse at 50% 50%, #0d1525 0%, #070b14 50%, #030508 100%);
  }

  /* Star field */
  .starfield {
    position: absolute;
    inset: 0;
    pointer-events: none;
  }

  .star {
    position: absolute;
    background: white;
    border-radius: 50%;
    animation: twinkle ease-in-out infinite alternate;
  }

  @keyframes twinkle {
    0% { opacity: 0.2; }
    100% { opacity: 1; }
  }

  /* Orbital system */
  .orbit-system {
    position: absolute;
    width: 400px;
    height: 400px;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -55%);
  }

  .orbits {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
  }

  .orbit-path {
    fill: none;
    stroke-width: 0.5;
    stroke-dasharray: 4 6;
    opacity: 0.15;
  }

  .orbit-1 {
    stroke: #6ee7b7;
    transform: rotate(-15deg);
    animation: dash 20s linear infinite;
  }
  .orbit-2 {
    stroke: #a5b4fc;
    transform: rotate(25deg);
    animation: dash 28s linear infinite reverse;
  }
  .orbit-3 {
    stroke: #fbbf24;
    transform: rotate(-45deg);
    animation: dash 35s linear infinite;
  }

  @keyframes dash {
    to { stroke-dashoffset: -100; }
  }

  /* Central portal */
  .portal {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: 40px;
    height: 40px;
  }

  .portal-core {
    position: absolute;
    inset: 0;
    border-radius: 50%;
    background: radial-gradient(circle, rgba(96, 165, 250, 0.8) 0%, rgba(96, 165, 250, 0.1) 60%, transparent 100%);
    animation: breathe 4s ease-in-out infinite;
  }

  .portal-ring {
    position: absolute;
    inset: -10px;
    border-radius: 50%;
    border: 1px solid rgba(96, 165, 250, 0.15);
    animation: breathe 4s ease-in-out infinite;
  }

  .portal-ring.ring-2 {
    inset: -24px;
    border-color: rgba(96, 165, 250, 0.07);
    animation-delay: 1s;
  }

  @keyframes breathe {
    0%, 100% { transform: scale(1); opacity: 0.6; }
    50% { transform: scale(1.12); opacity: 1; }
  }

  /* Orbiting stars */
  .star-orbit {
    position: absolute;
    top: 50%;
    left: 50%;
    width: 0;
    height: 0;
  }

  .orbit-alpha {
    animation: orbit-a 7s linear infinite;
  }
  .orbit-beta {
    animation: orbit-b 11s linear infinite;
  }
  .orbit-proxima {
    animation: orbit-c 17s linear infinite;
  }

  @keyframes orbit-a {
    from { transform: rotate(0deg) translateX(100px) rotate(0deg); }
    to { transform: rotate(360deg) translateX(100px) rotate(-360deg); }
  }
  @keyframes orbit-b {
    from { transform: rotate(120deg) translateX(120px) rotate(-120deg); }
    to { transform: rotate(480deg) translateX(120px) rotate(-480deg); }
  }
  @keyframes orbit-c {
    from { transform: rotate(240deg) translateY(-110px) rotate(-240deg); }
    to { transform: rotate(600deg) translateY(-110px) rotate(-600deg); }
  }

  .orbiting-star {
    position: relative;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    transform: translate(-50%, -50%);
  }

  .orbiting-star.alpha {
    background: #6ee7b7;
    box-shadow: 0 0 12px 4px rgba(110, 231, 183, 0.4), 0 0 30px 8px rgba(110, 231, 183, 0.15);
  }
  .orbiting-star.beta {
    background: #a5b4fc;
    box-shadow: 0 0 12px 4px rgba(165, 180, 252, 0.4), 0 0 30px 8px rgba(165, 180, 252, 0.15);
  }
  .orbiting-star.proxima {
    background: #fbbf24;
    box-shadow: 0 0 12px 4px rgba(251, 191, 36, 0.4), 0 0 30px 8px rgba(251, 191, 36, 0.15);
  }

  .star-label {
    position: absolute;
    top: 12px;
    left: 50%;
    transform: translateX(-50%);
    font-size: 0.6rem;
    white-space: nowrap;
    opacity: 0.5;
    letter-spacing: 0.05em;
    pointer-events: none;
  }

  .orbiting-star.alpha .star-label { color: #6ee7b7; }
  .orbiting-star.beta .star-label { color: #a5b4fc; }
  .orbiting-star.proxima .star-label { color: #fbbf24; }

  /* Content */
  .content {
    position: relative;
    z-index: 10;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 0.3rem;
    margin-top: 160px;
  }

  .title {
    font-size: 2.8rem;
    font-weight: 200;
    letter-spacing: 0.15em;
    margin: 0;
    background: linear-gradient(135deg, #60a5fa 0%, #a5b4fc 50%, #6ee7b7 100%);
    -webkit-background-clip: text;
    -webkit-text-fill-color: transparent;
    background-clip: text;
  }

  .subtitle {
    color: #475569;
    margin: 0;
    font-size: 0.9rem;
    letter-spacing: 0.08em;
  }

  .tagline {
    color: #334155;
    margin: 0;
    font-style: italic;
    font-size: 0.85rem;
    margin-bottom: 1.5rem;
  }

  .open-btn {
    position: relative;
    padding: 0.7rem 2.2rem;
    background: rgba(30, 41, 59, 0.6);
    border: 1px solid rgba(96, 165, 250, 0.2);
    color: #e2e8f0;
    border-radius: 10px;
    font-family: inherit;
    font-size: 0.95rem;
    cursor: pointer;
    transition: all 0.3s ease;
    overflow: hidden;
    backdrop-filter: blur(8px);
    letter-spacing: 0.05em;
  }

  .open-btn:hover {
    border-color: rgba(96, 165, 250, 0.5);
    background: rgba(30, 41, 59, 0.8);
    box-shadow: 0 0 20px rgba(96, 165, 250, 0.15);
  }

  .btn-glow {
    position: absolute;
    inset: 0;
    background: radial-gradient(circle at 50% 50%, rgba(96, 165, 250, 0.1) 0%, transparent 70%);
    opacity: 0;
    transition: opacity 0.3s;
  }

  .open-btn:hover .btn-glow {
    opacity: 1;
  }

  .trinity {
    margin-top: 2.5rem;
    display: flex;
    gap: 0.6rem;
    align-items: center;
    font-size: 0.75rem;
    letter-spacing: 0.1em;
  }

  .tri-local { color: #6ee7b7; }
  .tri-cloud { color: #a5b4fc; }
  .tri-emerge { color: #fbbf24; }
  .tri-op { color: #334155; }
</style>
