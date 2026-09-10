const reduceMotion = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
const header = document.querySelector("[data-header]");
const menuToggle = document.querySelector("[data-menu-toggle]");
const navigation = document.querySelector("[data-nav]");
const revealTargets = document.querySelectorAll(".reveal");
const steps = [...document.querySelectorAll("[data-step]")];
const stages = [...document.querySelectorAll("[data-stage]")];

const setActiveStep = (index) => {
  steps.forEach((step, position) => step.classList.toggle("is-active", position === index));
  stages.forEach((stage, position) => stage.classList.toggle("is-active", position === index));
};

if (!reduceMotion && "IntersectionObserver" in window) {
  const revealObserver = new IntersectionObserver(
  (entries) => entries.forEach((entry) => {
    if (entry.isIntersecting) {
      entry.target.classList.add("revealed");
      revealObserver.unobserve(entry.target);
    }
  }),
  { threshold: 0.12, rootMargin: "0px 0px -8% 0px" },
);
  revealTargets.forEach((target) => revealObserver.observe(target));
  document.documentElement.classList.add("motion-ready");
}

if ("IntersectionObserver" in window) {
  const stepObserver = new IntersectionObserver(
  (entries) => entries.forEach((entry) => {
    if (entry.isIntersecting) setActiveStep(Number(entry.target.dataset.step));
  }),
  { threshold: 0.56, rootMargin: "-12% 0px -22% 0px" },
);
  steps.forEach((step) => stepObserver.observe(step));
}

const updateHeader = () => header.classList.toggle("is-scrolled", window.scrollY > 12);
window.addEventListener("scroll", updateHeader, { passive: true });
updateHeader();

const closeMenu = () => {
  navigation?.classList.remove("is-open");
  menuToggle?.setAttribute("aria-expanded", "false");
};
menuToggle?.addEventListener("click", () => {
  const open = !navigation.classList.contains("is-open");
  navigation.classList.toggle("is-open", open);
  menuToggle.setAttribute("aria-expanded", String(open));
});
navigation?.querySelectorAll("a").forEach((link) => link.addEventListener("click", closeMenu));
window.addEventListener("resize", () => {
  if (window.innerWidth > 720) closeMenu();
}, { passive: true });
window.addEventListener("keydown", (event) => {
  if (event.key === "Escape") {
    closeMenu();
    menuToggle?.focus();
  }
});

if (!reduceMotion) {
  const floating = document.querySelectorAll("[data-float]");
  let frame;
  const animateFloating = () => {
    const time = performance.now() / 1000;
    floating.forEach((item) => {
      const delay = Number(item.dataset.delay || 0);
      item.style.translate = `0 ${Math.sin(time * 1.25 + delay) * 4}px`;
    });
    frame = requestAnimationFrame(animateFloating);
  };
  frame = requestAnimationFrame(animateFloating);
  window.addEventListener("pagehide", () => cancelAnimationFrame(frame), { once: true });
}
