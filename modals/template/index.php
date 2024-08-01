<div data-window='template_window' class='window window_bg' style='width: 330px; background: #bba229;'>
    <div data-part='handle' class='window_title' style='background-image: radial-gradient(#a18b21 1px, transparent 0) !important;'>
        <div class='float-right'>
            <button class="icon close_dark mr-1 hint--left" aria-label="Close (ESC)" data-close></button>
        </div>
        <div data-part='title' class='title_bg window_border' style='background: #bba229; color: #ede8d6;'>Blank template</div>
    </div>
    <div class='clearfix'></div>
    <div class='relative'>
        <div class='container text-light window_body p-2'>
            <div id="template_window_tabs">
                <div id="tabs" class="flex border-b border-gray-300">
                    <button class="tab text-gray-800" data-tab="tab1">Tab 1</button>
                    <button class="tab text-gray-800" data-tab="tab2">Tab 2</button>
                    <button class="tab text-gray-800" data-tab="tab3">Tab 3</button>
                </div>

                <div class="tab-content p-4 hidden" data-tab-content="tab1">
                    <div id="template_window_tabs2">
                        <div id="tabs" class="flex border-b border-gray-300">
                            <button class="tab text-gray-800" data-tab="tab1">Tab 1</button>
                            <button class="tab text-gray-800" data-tab="tab2">Tab 2</button>
                            <button class="tab text-gray-800" data-tab="tab3">Tab 3</button>
                        </div>

                        <div class="tab-content p-4 hidden" data-tab-content="tab1">
                            <p>Content for Tab 1</p>
                        </div>

                        <div class="tab-content p-4 hidden" data-tab-content="tab2">
                            <p>Content for Tab 2</p>
                        </div>

                        <div class="tab-content p-4 hidden" data-tab-content="tab3">
                            <p>Content for Tab 3</p>
                        </div>
                    </div>
                </div>

                <div class="tab-content p-4 hidden" data-tab-content="tab2">
                    <p>Content for Tab 2</p>
                </div>

                <div class="tab-content p-4 hidden" data-tab-content="tab3">
                    <p>Content for Tab 3</p>
                </div>
            </div>
        </div>
    </div>

    <script>
        var template_window = {
            start: function() {
                ui.initTabs('template_window_tabs', 'tab2');
                ui.initTabs('template_window_tabs2', 'tab2');
            },
            unmount: function() {
                ui.destroyTabs('template_window_tabs');
                ui.destroyTabs('template_window_tabs2');
            },
            buttonA: function(e) {

            },
            buttonB: function(e) {

            },
            buttonStart: function(e) {

            },
            leftAxis: function(e) {

            },
            leftTrigger: function(e) {

            },
            leftBumper: function(e) {

            }
        }
        template_window.start();
    </script>

    <div class='resize-handle'></div>
</div>
