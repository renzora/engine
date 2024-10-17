<?php
include $_SERVER['DOCUMENT_ROOT'] . '/config.php';
if ($auth) {
?>
  <div data-window='yamltojson_window' class='window bg-yellow-700' style='width: 600px;'>

    <div data-part='handle' class='window_title bg-yellow-600 text-yellow-100 p-2 rounded-t'>
      <div class='float-right'>
        <button class="icon close_dark mr-1 text-white" aria-label="Close (ESC)" data-close>&times;</button>
      </div>
      <div data-part='title' class='title_bg window_border text-yellow-100'>YAML to JSON Converter</div>
    </div>
    
    <div class='clearfix'></div>
    
    <div class='relative'>
      <div class='window_body container text-white p-2'>
        <p>Paste your YAML content below and it will be converted to JSON.</p>
        <textarea id="yaml_input" class="w-full p-2 text-black" rows="10" placeholder="Enter YAML here"></textarea>
        <button id="convert_button" class="mt-2 bg-yellow-600 text-white p-2 rounded">Convert to JSON</button>
        <button id="copy_button" class="mt-2 ml-2 bg-yellow-600 text-white p-2 rounded">Copy to Clipboard</button>
        <pre id="json_output" class="bg-gray-800 text-white p-2 mt-2" style="height: 300px; overflow-y: auto;"></pre>
      </div>
    </div>

    <script>
var yamltojson_window = {
    start: function() {
        // Event listener for conversion button
        document.getElementById('convert_button').addEventListener('click', function() {
            var yamlInput = document.getElementById('yaml_input').value;
            console.log("Original YAML Input:\n", yamlInput);

            var jsonOutput = yamltojson_window.parseYamlToJSON(yamlInput);
            console.log("Parsed JSON Output:\n", JSON.stringify(jsonOutput, null, 2));

            document.getElementById('json_output').textContent = JSON.stringify(jsonOutput, null, 2);
        });

        // Event listener for copy to clipboard button
        document.getElementById('copy_button').addEventListener('click', function() {
            var jsonOutput = document.getElementById('json_output').textContent;
            yamltojson_window.copyToClipboard(jsonOutput);
        });
    },

    unmount: function() {
        // Clean up code if necessary
    },

    // Function to parse YAML into a JSON object
    parseYamlToJSON: function(yaml) {
        const lines = yaml.split('\n');
            const result = {};
            let currentObject = result;
            let objectStack = [result];
            let indentStack = [0]; // Stack to track indentation levels
            let previousIndent = 0; // Track the previous indentation level
            let lastKey = ''; // Track the last key parsed
        
            lines.forEach(line => {
                if (line.trim() === '') return; // Skip empty lines
        
                const indent = line.search(/\S/); // Find the current line's indentation level
                const cleanLine = line.trim(); // Clean the line of leading/trailing spaces
        
                // If moving back to a sibling (same indentation as previous), pop the stack
                if (indent < previousIndent && objectStack.length > 1) {
                    while (indent <= indentStack[indentStack.length - 1]) {
                        objectStack.pop();
                        indentStack.pop();
                    }
                    currentObject = objectStack[objectStack.length - 1];
                }
        
                if (cleanLine.startsWith('- ')) {
                    const listItem = cleanLine.slice(2).trim().replace(/^["']|["']$/g, ''); // Remove quotes
                    if (Array.isArray(currentObject[lastKey])) {
                        currentObject[lastKey].push(listItem);
                    } else {
                        currentObject[lastKey] = [listItem]; // Convert to array if not already
                    }
                } else if (cleanLine.includes(':')) {
                    const [rawKey, ...rawValue] = cleanLine.split(':');
                    const key = rawKey.trim();
                    let value = rawValue.join(':').trim().replace(/^["']|["']$/g, ''); // Remove quotes
        
                    if (value === '') {
                        // If the value is empty (indicating a nested object), create a new object
                        currentObject[key] = {};
                        objectStack.push(currentObject[key]); // Move into the new object and push to stack
                        currentObject = currentObject[key];
                        indentStack.push(indent); // Track this indentation level
                    } else {
                        // Otherwise, assign the value
                        currentObject[key] = value;
                    }
                    lastKey = key;
        
                }
        
                // Update the previous indentation level for the next line
                previousIndent = indent;
            });
        
            return result; // Return the final parsed object
},


    // Utility function to copy text to clipboard
    copyToClipboard: function(text) {
        const tempTextArea = document.createElement('textarea');
        tempTextArea.value = text;
        document.body.appendChild(tempTextArea);
        tempTextArea.select();
        document.execCommand('copy');
        document.body.removeChild(tempTextArea);
        alert('JSON copied to clipboard!');
    }
}

yamltojson_window.start();


    </script>

  </div>
<?php
}
?>
