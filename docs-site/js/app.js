// ============================================================================
// Alpha Documentation Book - Core Application Logic (Minimalist Monochrome)
// ============================================================================

const SVG_GLOBE = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="2" y1="12" x2="22" y2="12"></line><path d="M12 2a15.3 15.3 0 0 1 4 10 15.3 15.3 0 0 1-4 10 15.3 15.3 0 0 1-4-10 15.3 15.3 0 0 1 4-10z"></path></svg>`;
const SVG_MOON = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M21 12.79A9 9 0 1 1 11.21 3 7 7 0 0 0 21 12.79z"></path></svg>`;
const SVG_SUN = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="5"></circle><line x1="12" y1="1" x2="12" y2="3"></line><line x1="12" y1="21" x2="12" y2="23"></line><line x1="4.22" y1="4.22" x2="5.64" y2="5.64"></line><line x1="18.36" y1="18.36" x2="19.78" y2="19.78"></line><line x1="1" y1="12" x2="3" y2="12"></line><line x1="21" y1="12" x2="23" y2="12"></line><line x1="4.22" y1="19.78" x2="5.64" y2="18.36"></line><line x1="18.36" y1="5.64" x2="19.78" y2="4.22"></line></svg>`;

(function() {
  let currentLang = localStorage.getItem('alpha_docs_lang') || 'en';
  let currentTheme = localStorage.getItem('alpha_docs_theme') || 'dark';
  let currentChapterId = 'intro';

  // Apply theme
  document.documentElement.setAttribute('data-theme', currentTheme);

  function getChapters() {
    return currentLang === 'ru' ? window.ALPHA_CHAPTERS_RU : window.ALPHA_CHAPTERS_EN;
  }

  function getLabels() {
    return currentLang === 'ru' ? {
      docs: "Документация",
      searchPlaceholder: "Поиск по книге... (Ctrl + K)",
      wasHelpful: "Была ли эта страница полезна?",
      thanksFeedback: "Спасибо за ваш отзыв!",
      onThisPage: "НА ЭТОЙ СТРАНИЦЕ",
      previous: "Назад",
      next: "Далее",
      noResults: "Ничего не найдено",
      copy: "Копировать",
      copied: "Скопировано!"
    } : {
      docs: "Documentation",
      searchPlaceholder: "Search docs... (Ctrl + K)",
      wasHelpful: "Was this helpful?",
      thanksFeedback: "Thanks for your feedback!",
      onThisPage: "ON THIS PAGE",
      previous: "Previous",
      next: "Next",
      noResults: "No matching results",
      copy: "Copy",
      copied: "Copied!"
    };
  }

  // Render Sidebar (Removed 'Powered by alpha engine' per user request)
  function renderSidebar() {
    const chapters = getChapters();
    const sidebarEl = document.getElementById('sidebarNav');
    if (!sidebarEl) return;

    // Group chapters by group name
    const groups = {};
    chapters.forEach(ch => {
      if (!groups[ch.group]) groups[ch.group] = [];
      groups[ch.group].push(ch);
    });

    let html = '';
    for (const [groupName, items] of Object.entries(groups)) {
      html += `<div class="sidebar-group">
        <div class="sidebar-group-title">${groupName}</div>`;
      items.forEach(item => {
        const isActive = item.id === currentChapterId ? 'active' : '';
        html += `<a href="#${item.id}" class="sidebar-nav-item ${isActive}" data-id="${item.id}">
          <span>${item.title}</span>
        </a>`;
      });
      html += `</div>`;
    }

    sidebarEl.innerHTML = html;
  }

  // Render Current Chapter
  function renderChapter() {
    const chapters = getChapters();
    const labels = getLabels();
    let chapterIndex = chapters.findIndex(c => c.id === currentChapterId);
    if (chapterIndex === -1) {
      chapterIndex = 0;
      currentChapterId = chapters[0].id;
    }
    const chapter = chapters[chapterIndex];

    // Update Breadcrumbs
    const breadcrumbsEl = document.getElementById('breadcrumbs');
    if (breadcrumbsEl) {
      breadcrumbsEl.innerHTML = `
        <a href="#intro">${labels.docs}</a>
        <span class="separator">/</span>
        <span>${chapter.group}</span>
        <span class="separator">/</span>
        <span>${chapter.title}</span>
      `;
    }

    // Update Title & Subtitle
    const titleEl = document.getElementById('chapterTitle');
    if (titleEl) titleEl.innerText = chapter.title;

    const subtitleEl = document.getElementById('chapterSubtitle');
    if (subtitleEl) subtitleEl.innerText = chapter.subtitle;

    // Update Body Content
    const bodyEl = document.getElementById('chapterBody');
    if (bodyEl) {
      bodyEl.innerHTML = chapter.body;
    }

    // Update Feedback Text & Buttons
    const feedbackLabelEl = document.getElementById('feedbackLabel');
    if (feedbackLabelEl) feedbackLabelEl.innerText = labels.wasHelpful;

    const feedbackButtonsEl = document.getElementById('feedbackButtons');
    if (feedbackButtonsEl) {
      feedbackButtonsEl.innerHTML = `
        <button class="feedback-btn" onclick="sendFeedback('up')" title="Helpful">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M14 9V5a3 3 0 0 0-3-3l-4 9v11h11.28a2 2 0 0 0 2-1.7l1.38-9a2 2 0 0 0-2-2.3zM7 22H4a2 2 0 0 1-2-2v-7a2 2 0 0 1 2-2h3"></path></svg>
        </button>
        <button class="feedback-btn" onclick="sendFeedback('neutral')" title="Neutral">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><circle cx="12" cy="12" r="10"></circle><line x1="8" y1="12" x2="16" y2="12"></line></svg>
        </button>
        <button class="feedback-btn" onclick="sendFeedback('down')" title="Not Helpful">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round"><path d="M10 15v4a3 3 0 0 0 3 3l4-9V2H5.72a2 2 0 0 0-2 1.7l-1.38 9a2 2 0 0 0 2 2.3zm7-13h3a2 2 0 0 1 2 2v7a2 2 0 0 1-2 2h-3"></path></svg>
        </button>
      `;
    }

    // Update Pagination (Prev / Next book cards)
    const paginationEl = document.getElementById('paginationNav');
    if (paginationEl) {
      const prevCh = chapterIndex > 0 ? chapters[chapterIndex - 1] : null;
      const nextCh = chapterIndex < chapters.length - 1 ? chapters[chapterIndex + 1] : null;

      let pagHtml = '';
      if (prevCh) {
        pagHtml += `
          <a href="#${prevCh.id}" class="page-card prev">
            <span class="page-card-hint">← ${labels.previous}</span>
            <span class="page-card-title">${prevCh.title}</span>
          </a>
        `;
      } else {
        pagHtml += `<div></div>`;
      }

      if (nextCh) {
        pagHtml += `
          <a href="#${nextCh.id}" class="page-card next">
            <span class="page-card-hint">${labels.next} →</span>
            <span class="page-card-title">${nextCh.title}</span>
          </a>
        `;
      } else {
        pagHtml += `<div></div>`;
      }
      paginationEl.innerHTML = pagHtml;
    }

    // Update Right TOC
    renderTOC();

    // Scroll to top
    window.scrollTo({ top: 0, behavior: 'smooth' });

    // Update active highlight in sidebar
    document.querySelectorAll('.sidebar-nav-item').forEach(el => {
      if (el.getAttribute('data-id') === currentChapterId) {
        el.classList.add('active');
      } else {
        el.classList.remove('active');
      }
    });

    // Update active highlight in top header nav
    const tabDocs = document.getElementById('navTabDocs');
    const tabStd = document.getElementById('navTabStdlib');
    const tabWeb = document.getElementById('navTabWebalph');
    if (tabDocs && tabStd && tabWeb) {
      tabDocs.classList.remove('active');
      tabStd.classList.remove('active');
      tabWeb.classList.remove('active');
      if (currentChapterId === 'stdlib-cores') {
        tabStd.classList.add('active');
      } else if (currentChapterId === 'webalph-tooling') {
        tabWeb.classList.add('active');
      } else {
        tabDocs.classList.add('active');
      }
    }

    // Update page title
    document.title = `${chapter.title} - Alpha Docs`;
  }

  // Render Right Table of Contents (TOC)
  function renderTOC() {
    const tocListEl = document.getElementById('tocList');
    const tocTitleEl = document.getElementById('tocTitle');
    if (!tocListEl) return;

    const labels = getLabels();
    if (tocTitleEl) tocTitleEl.innerText = labels.onThisPage;

    const headings = document.querySelectorAll('#chapterBody h2');
    if (!headings || headings.length === 0) {
      tocListEl.innerHTML = `<li class="toc-link" style="color:var(--text-muted); font-size:12px;">-</li>`;
      return;
    }

    let html = '';
    headings.forEach((h, idx) => {
      const id = 'heading-' + idx;
      h.id = id;
      html += `<li><a href="#${id}" class="toc-link" onclick="scrollToHeading('${id}', event)">${h.innerText}</a></li>`;
    });
    tocListEl.innerHTML = html;
  }

  window.scrollToHeading = function(id, evt) {
    if (evt) evt.preventDefault();
    const el = document.getElementById(id);
    if (el) {
      const top = el.getBoundingClientRect().top + window.scrollY - 80;
      window.scrollTo({ top, behavior: 'smooth' });
    }
  };

  // Language Switcher (Vector SVG + Text)
  window.toggleLanguage = function() {
    currentLang = currentLang === 'en' ? 'ru' : 'en';
    localStorage.setItem('alpha_docs_lang', currentLang);
    updateLangUI();
    renderSidebar();
    renderChapter();
  };

  function updateLangUI() {
    const btn = document.getElementById('langSwitchBtn');
    if (btn) {
      btn.innerHTML = `${SVG_GLOBE} <span>${currentLang.toUpperCase()}</span>`;
    }
    const searchInput = document.getElementById('searchTriggerText');
    if (searchInput) {
      searchInput.innerText = getLabels().searchPlaceholder;
    }
    const tabDocs = document.getElementById('navTabDocs');
    const tabStd = document.getElementById('navTabStdlib');
    const tabWeb = document.getElementById('navTabWebalph');
    if (tabDocs) tabDocs.innerText = currentLang === 'ru' ? "Документация" : "Documentation";
    if (tabStd) tabStd.innerText = currentLang === 'ru' ? "Стандартная библиотека" : "Standard Library";
    if (tabWeb) tabWeb.innerText = "WebAlph";
  }

  // Theme Toggle (Vector SVG)
  window.toggleTheme = function() {
    currentTheme = currentTheme === 'dark' ? 'light' : 'dark';
    document.documentElement.setAttribute('data-theme', currentTheme);
    localStorage.setItem('alpha_docs_theme', currentTheme);
    updateThemeUI();
  };

  function updateThemeUI() {
    const btn = document.getElementById('themeToggleBtn');
    if (btn) {
      btn.innerHTML = currentTheme === 'dark' ? SVG_MOON : SVG_SUN;
    }
  }

  // Feedback Click
  window.sendFeedback = function(type) {
    const box = document.getElementById('feedbackButtons');
    if (box) {
      box.innerHTML = `<span style="color:var(--text-primary); font-size:13px; font-weight:600;">${getLabels().thanksFeedback}</span>`;
    }
  };

  // Copy Code to Clipboard
  window.copyCode = function(button) {
    const container = button.closest('.code-block-container');
    if (!container) return;
    const codeEl = container.querySelector('code');
    if (!codeEl) return;

    navigator.clipboard.writeText(codeEl.innerText).then(() => {
      const origText = button.innerText;
      button.innerText = getLabels().copied;
      button.style.color = "var(--text-primary)";
      setTimeout(() => {
        button.innerText = origText;
        button.style.color = "";
      }, 2000);
    });
  };

  // Route Handling (Hash Change)
  function handleRoute() {
    const hash = window.location.hash.replace('#', '');
    if (hash && !hash.startsWith('heading-')) {
      currentChapterId = hash;
    }
    renderChapter();
  }

  window.addEventListener('hashchange', handleRoute);

  // Search Modal
  const modal = document.getElementById('searchModal');
  const searchInput = document.getElementById('searchModalInput');
  const searchResults = document.getElementById('searchResults');

  window.openSearch = function() {
    if (modal) {
      modal.classList.add('open');
      if (searchInput) {
        searchInput.value = '';
        searchInput.focus();
        renderSearchResults('');
      }
    }
  };

  window.closeSearch = function() {
    if (modal) modal.classList.remove('open');
  };

  window.addEventListener('keydown', (e) => {
    if ((e.ctrlKey || e.metaKey) && e.key.toLowerCase() === 'k') {
      e.preventDefault();
      openSearch();
    }
    if (e.key === 'Escape' && modal && modal.classList.contains('open')) {
      closeSearch();
    }
  });

  if (searchInput) {
    searchInput.addEventListener('input', (e) => {
      renderSearchResults(e.target.value.trim());
    });
  }

  function renderSearchResults(q) {
    if (!searchResults) return;
    if (!q) {
      searchResults.innerHTML = `<div style="padding:20px; text-align:center; color:var(--text-muted); font-size:13px;">Type keywords like <code>vect</code>, <code>strict</code>, <code>paint</code>, <code>webalph</code>...</div>`;
      return;
    }

    const query = q.toLowerCase();
    const chapters = getChapters();
    const matches = [];

    chapters.forEach(ch => {
      const titleMatch = ch.title.toLowerCase().includes(query);
      const subMatch = ch.subtitle.toLowerCase().includes(query);
      const bodyClean = ch.body.replace(/<[^>]+>/g, ' ');
      const bodyIndex = bodyClean.toLowerCase().indexOf(query);

      if (titleMatch || subMatch || bodyIndex !== -1) {
        let snippet = ch.subtitle;
        if (bodyIndex !== -1) {
          const start = Math.max(0, bodyIndex - 40);
          const end = Math.min(bodyClean.length, bodyIndex + 80);
          snippet = "..." + bodyClean.substring(start, end) + "...";
        }
        matches.push({ id: ch.id, title: ch.title, snippet });
      }
    });

    if (matches.length === 0) {
      searchResults.innerHTML = `<div style="padding:20px; text-align:center; color:var(--text-muted); font-size:13px;">${getLabels().noResults}</div>`;
      return;
    }

    let html = '';
    matches.forEach(m => {
      html += `
        <div class="search-result-item" onclick="selectSearchResult('${m.id}')">
          <div class="search-result-title">${m.title}</div>
          <div class="search-result-excerpt">${m.snippet}</div>
        </div>
      `;
    });
    searchResults.innerHTML = html;
  }

  window.selectSearchResult = function(id) {
    closeSearch();
    window.location.hash = '#' + id;
  };

  // Init
  document.addEventListener('DOMContentLoaded', () => {
    updateLangUI();
    updateThemeUI();
    const hash = window.location.hash.replace('#', '');
    if (hash && !hash.startsWith('heading-')) {
      currentChapterId = hash;
    }
    renderSidebar();
    renderChapter();
  });
})();
