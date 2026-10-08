// Plays a looping video only while it is on screen. A person's pause wins over scrolling, reduced motion and
// Save-Data never autoplay, and a failed load falls back to the poster with a Play button (WCAG 2.2.2).
// `root` holds one <video> and one .video-toggle button (VideoToggle.astro).
export function autoplay(root: HTMLElement): void {
  const video = root.querySelector("video")!;
  const btn = root.querySelector<HTMLButtonElement>(".video-toggle")!;
  const saveData = (navigator as Navigator & { connection?: { saveData?: boolean } }).connection?.saveData === true;
  const auto = !matchMedia("(prefers-reduced-motion: reduce)").matches && !saveData;
  let userPaused = !auto;
  let failed = false;

  const show = (playing: boolean) => {
    btn.classList.toggle("is-playing", playing);
    btn.setAttribute("aria-label", (playing ? btn.dataset.pause : btn.dataset.play) ?? "");
  };
  const play = () => {
    if (failed) return;
    video.play().then(() => show(true), () => show(false));
  };
  const pause = () => {
    video.pause();
    show(false);
  };

  // Only the media element's own error is fatal. A <source> error just means the browser moves on to the
  // next source (e.g. no WebM support), so it must not stop playback of the MP4.
  video.addEventListener("error", () => { failed = true; show(false); });
  btn.addEventListener("click", () => {
    if (video.paused) {
      userPaused = false;
      play();
    } else {
      userPaused = true;
      pause();
    }
  });
  new IntersectionObserver(
    ([e]) => {
      if (e.isIntersecting && !userPaused) play();
      if (!e.isIntersecting && !video.paused) pause();
    },
    // A quarter in view is enough: at 1440×900 the English hero's longer headline leaves only ~40% of the video
    // above the fold, and a 0.4 threshold never fired until the person scrolled.
    { threshold: 0.25 },
  ).observe(video);

  btn.hidden = false;
  show(false);
}
