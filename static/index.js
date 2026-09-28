function initIndexPage() {
  const scrollRoot = document.getElementById("index-scroll");
  const rail = document.getElementById("letter-rail");
  const marker = document.getElementById("rail-marker");
  if (!scrollRoot || !rail) return;

  const railLinks = Array.from(rail.querySelectorAll("[data-letter]"));
  const sections = Array.from(document.querySelectorAll("[data-letter-section]"));
  if (!railLinks.length || !sections.length) return;

  // The rail only renders at the lg breakpoint; skip all the tracking
  // work otherwise, since links/scroll aren't in view to style.
  const railQuery = window.matchMedia("(min-width: 1024px)");

  let activeIndex = -1;

  function applyState(index) {
    function applyLinkState(link, i) {
      const dist = Math.abs(i - index);
      if (dist === 0) {
        link.style.color = "var(--color-gold)";
        link.style.transform = "scale(1.6)";
      } else if (dist === 1) {
        link.style.color = "var(--color-lexicon)";
        link.style.transform = "scale(1.2)";
      } else {
        link.style.color = "";
        link.style.transform = "scale(1)";
      }
    }
    railLinks.forEach(applyLinkState);

    const activeLink = railLinks[index];
    if (marker && activeLink) {
      marker.style.top = `${activeLink.offsetTop + activeLink.offsetHeight / 2}px`;
      marker.style.opacity = "1";
    }
  }

  function updateActive() {
    if (!railQuery.matches) return;
    const probeY = scrollRoot.getBoundingClientRect().top + 120;
    let current = 0;
    for (let i = 0; i < sections.length; i++) {
      if (sections[i].getBoundingClientRect().top <= probeY) {
        current = i;
      } else {
        break;
      }
    }
    if (current !== activeIndex) {
      activeIndex = current;
      applyState(activeIndex);
    }
  }

  let ticking = false;
  function onScroll() {
    function tick() {
      updateActive();
      ticking = false;
    }
    if (!ticking) {
      window.requestAnimationFrame(tick);
      ticking = true;
    }
  }

  scrollRoot.addEventListener("scroll", onScroll, { passive: true });
  window.addEventListener("resize", onScroll);
  if (railQuery.addEventListener) {
    railQuery.addEventListener("change", updateActive);
  }
  updateActive();

  // The display serif swaps in after first paint and grows the giant
  // section letters, which shifts the whole list down mid-jump. Re-settle
  // onto the intended target once fonts are actually done loading.
  function resettle(id) {
    function onFontsReady() {
      const target = document.getElementById(id);
      if (target) target.scrollIntoView({ block: "start" });
      updateActive();
    }
    if (!document.fonts?.ready) return;
    document.fonts.ready.then(onFontsReady);
  }

  if (window.location.hash) {
    resettle(window.location.hash.slice(1));
  }

  function bindRailLink(link) {
    function onClick() {
      resettle(link.getAttribute("href").slice(1));
    }
    link.addEventListener("click", onClick);
  }
  railLinks.forEach(bindRailLink);
}

initIndexPage();
