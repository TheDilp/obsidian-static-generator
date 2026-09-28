const input = document.getElementById("page-search-input");
function searchPages(e) {
  if (!input) return;
  const value = String(e?.currentTarget?.value || "");
  const allSections = document.getElementsByTagName("section");
  const allListItems = document.getElementsByTagName("li");
  if (value?.length >= 3) {
    if (!allSections.length) return;

    for (const element of allListItems) {
      if (element.dataset.linkTitle) {
        const match = element.dataset.linkTitle
          .toLowerCase()
          .match(value.toLowerCase());
        if (!match?.length) {
          element.classList.add("hidden");
        }
      }
    }
    for (const section of allSections) {
      const listItemArray = Array.from(section.getElementsByTagName("li"));
      if (listItemArray.every((li) => li.classList.contains("hidden"))) {
        section.classList.add("hidden");
      }
    }
  } else {
    for (let index = 0; index < (allSections?.length || 0); index++) {
      allSections[index].classList.remove("hidden");
    }

    for (let index = 0; index < (allListItems?.length || 0); index++) {
      allListItems[index].classList.remove("hidden");
    }
  }
}

input.addEventListener("input", searchPages);
