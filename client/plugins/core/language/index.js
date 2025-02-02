language = {
    isTranslated: false,
    currentLang: '',
    textNodes: new Map(),
    dictionaries: {},
    start() {
        console.log(`Plugin started: ${this.id}`);
        fetch('plugins/language/dictionary.json')
            .then(response => response.json())
            .then(data => {
                this.dictionaries = data;
                this.currentLang = localStorage.getItem('language') || 'en';
                this.addDropUp();
                this.updateTranslation();
            })
            .catch(error => {
                console.error("Error fetching dictionary:", error);
                this.currentLang = localStorage.getItem('language') || 'en';
                this.addDropUp();
                this.updateTranslation();
            });
    },
    unmount() {
        this.removeDropUp();
        this.restoreAllText();
        console.log(`Plugin unmounted: ${this.id}`);
    },
    addDropUp() {
        const container = document.createElement('div');
        container.id = 'language-dropup';
        container.className = 'fixed bottom-4 right-4 z-50';
        container.addEventListener('click', (e) => {
            e.stopPropagation();
        });
        const toggler = document.createElement('div');
        toggler.id = 'language-toggler';
        toggler.className = 'bg-white border window_body border-gray-300 text-gray-700 py-2 px-4 rounded cursor-pointer select-none';
        const languages = [
            { value: 'en', label: 'English' },
            { value: 'es', label: 'Spanish' },
            { value: 'fr', label: 'French' }
        ];
        const currentLangObj = languages.find(lang => lang.value === this.currentLang);
        if (currentLangObj) {
            toggler.innerText = currentLangObj.label;
        } else {
            toggler.innerText = 'Select Language';
        }
        const optionsContainer = document.createElement('div');
        optionsContainer.id = 'language-options';
        optionsContainer.className = 'hidden absolute bottom-full mb-2 right-0 bg-white border border-gray-300 rounded shadow-lg z-50';
        optionsContainer.style.minWidth = '150px';
        languages.forEach(lang => {
            const option = document.createElement('div');
            option.className = 'py-2 px-4 hover:bg-gray-100 cursor-pointer';
            option.innerText = lang.label;
            option.dataset.value = lang.value;
            option.addEventListener('click', (e) => {
                e.stopPropagation();
                this.currentLang = lang.value;
                localStorage.setItem('language', lang.value);
                toggler.innerText = lang.label;
                optionsContainer.classList.add('hidden');
                this.updateTranslation();
            });
            optionsContainer.appendChild(option);
        });
        toggler.addEventListener('click', (e) => {
            e.stopPropagation();
            optionsContainer.classList.toggle('hidden');
        });
        document.addEventListener('click', (e) => {
            if (!container.contains(e.target)) {
                optionsContainer.classList.add('hidden');
            }
        });
        container.appendChild(toggler);
        container.appendChild(optionsContainer);
        document.body.appendChild(container);
    },
    removeDropUp() {
        const container = document.getElementById('language-dropup');
        if (container) container.remove();
    },
    updateTranslation() {
        this.restoreAllText();
        if (this.currentLang) {
            this.translate(this.currentLang);
            this.isTranslated = true;
        } else {
            this.isTranslated = false;
        }
    },
    translate(lang) {
        const dictionary = this.dictionaries[lang];
        if (!dictionary) {
            alert("Dictionary for the selected language is not available");
            return;
        }
        const walker = document.createTreeWalker(
            document.body,
            NodeFilter.SHOW_TEXT,
            {
                acceptNode: (node) => {
                    if (!node.nodeValue.trim()) return NodeFilter.FILTER_REJECT;
                    if (node.parentNode && (node.parentNode.id === 'language-toggler' ||
                        node.parentNode.id === 'language-options' ||
                        node.parentNode.closest('#language-dropup')))
                        return NodeFilter.FILTER_REJECT;
                    return NodeFilter.FILTER_ACCEPT;
                }
            },
            false
        );
        let node;
        while (node = walker.nextNode()) {
            if (!this.textNodes.has(node)) {
                this.textNodes.set(node, node.nodeValue);
            }
            let originalText = this.textNodes.get(node);
            for (const key in dictionary) {
                const re = new RegExp(`\\{${this.escapeRegExp(key)}\\}`, 'g');
                originalText = originalText.replace(re, dictionary[key]);
            }
            node.nodeValue = originalText;
        }
    },
    restoreAllText() {
        this.textNodes.forEach((orig, node) => {
            node.nodeValue = orig;
        });
        this.textNodes.clear();
    },
    escapeRegExp(string) {
        return string.replace(/[.*+?^${}()|[\]\\]/g, '\\$&');
    }
};
