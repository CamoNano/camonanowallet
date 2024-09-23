// Advanced options
const advancedOptions = document.getElementById('advanced-options');
document.getElementById('toggle-advanced-options').addEventListener('click', () => {
    if (advancedOptions.style.display === 'none') {
        advancedOptions.style.display = 'block';
    } else {
        advancedOptions.style.display = 'none';
    }
});


///// Tab functionality /////
const tabs = document.querySelectorAll('.tab');
const tabContents = document.querySelectorAll('.tab-content');
tabs.forEach(tab => {
    tab.addEventListener('click', () => {
        // Remove active class from all tabs and contents
        tabs.forEach(t => t.classList.remove('active'));
        tabContents.forEach(tc => tc.classList.remove('active'));

        // Add active class to the clicked tab and corresponding content
        tab.classList.add('active');
        document.getElementById(tab.getAttribute('data-tab')).classList.add('active');
    });
});


///// Toggle Camo /////
const toggleCamo = document.getElementById('toggle-camo');
const sendRecipientLabel = document.getElementById('recipient-label');
const receiveAddress = document.getElementById("receive-address");
toggleCamo.addEventListener('change', function() {
    const recipient = document.getElementById('recipient');
    receiveAddress.innerHTML = "Select";

    if (this.checked) {
        sendRecipientLabel.innerHTML = 'Recipient (camo_ address):';
        recipient.placeholder = "Camo Address";
        setReceiveCamoAddressList();
    } else {
        sendRecipientLabel.innerHTML = 'Recipient (nano_ address):';
        recipient.placeholder = "Nano Address";
        setReceiveNanoAddressList();
    }
});


///// Dropdown Menus /////

// Set the values of a dropdown list
function setDropdownList(dropdownId, options) {
    const dropdown = document.getElementById(dropdownId);
    const optionsContainer = dropdown.querySelector('.options');

    // Clear existing options and their event listeners
    while (optionsContainer.firstChild) {
        optionsContainer.removeChild(optionsContainer.firstChild);
    }

    // Add new options
    options.forEach(item => {
        const option = document.createElement('div');
        option.classList.add('option');
        option.textContent = item;

        // Define the click event handler
        const handleOptionClick = () => {
            const selectedOption = dropdown.querySelector('.selected-option');
            selectedOption.textContent = item;
            optionsContainer.style.display = 'none';
        };

        // Attach click event to the option
        option.addEventListener('click', handleOptionClick);

        // Append the option to the container
        optionsContainer.appendChild(option);
    });
}

const dropdowns = document.querySelectorAll('.dropdown');

// Close the dropdown if the user clicks outside of it
window.addEventListener('click', (e) => {
    dropdowns.forEach(dropdown => {
        const selectedOption = dropdown.querySelector('.selected-option');
        const optionsContainer = dropdown.querySelector('.options');

        if (!selectedOption.contains(e.target) && !optionsContainer.contains(e.target)) {
            optionsContainer.style.display = 'none';
        }
    });
});

dropdowns.forEach(dropdown => {
    const selectedOption = dropdown.querySelector('.selected-option');
    const optionsContainer = dropdown.querySelector('.options');

    selectedOption.addEventListener('click', () => {
        optionsContainer.style.display = optionsContainer.style.display === 'block' ? 'none' : 'block';
    });
});


///// Receive Tab /////

// Switch between auto and manual index
const generateIndex = document.getElementById('generate-index');
const autoIndex = document.getElementById('auto-index-checkbox');
autoIndex.addEventListener('change', function() {
    if (this.checked) {
        generateIndex.disabled = true;
        generateIndex.value = 0;
    } else {
        generateIndex.disabled = false;
    }
});

// Generate address
document.getElementById('generate-address').addEventListener('click', () => {
    if (autoIndex.checked) {
        generateAddress(toggleCamo.checked);
    } else {
        generateAddress(toggleCamo.checked, Number(generateIndex.value));
    }
    alert("Address generated.");
});

// Copy address to clipboard
document.getElementById('copy-address').addEventListener('click', () => {
    const address = receiveAddress.textContent;
    if (address == 'Select') {
        alert('Please select an address first.');
        return
    }
    navigator.clipboard.writeText(address)
        .then(() => alert(`Copied: ${address}`))
        .catch(err => console.error('Failed to copy: ', err));
});
// Remove address
document.getElementById('remove-address').addEventListener('click', () => {
    const address = receiveAddress.textContent;
    if (address == 'Select') {
        alert('Please select an address first.');
        return
    }
    if (confirm(`Are you sure you want to remove ${address}?`)) {
        deleteAddress(toggleCamo.checked, address);
        alert("Address removed.");
    }
});


///// Wallet /////

// Get the index to generate an account at
function getGenerateIndex() {
    const generateIndex = document.getElementById('generate-index').value;
    if (generateIndex == "auto") {
        alert()
        return -1
    }
    if (generateIndex >= 0 && generateIndex < 2 ** 32) {
        return generateIndex
    }
    else {
        alert("Invalid index. Enter 'auto' or a valid index.");
    }
}

// Update balance function
function updateBalance(balance, pending_balance) {
    var value = `Wallet balance: ${balance} Nano`;
    if (pending_balance) {
        value += ` (+ ${pending_balance} Nano receivable)`;
    }
    document.getElementById('balance').textContent = value;
};

// Refresh the wallet
function refresh() {

}

// Create an address
function generateAddress(is_camo, index) {

}

// Delete an address
function deleteAddress(is_camo, address) {

}

// Set recipient nano_ addresses
function setReceiveNanoAddressList() {
    setDropdownList("receive-address-list", ["nano_1xh65p5t4amf8hcsasm7jyf1mrypkj14thneh5wxuk14c6p997wf4ihoxdhp", "nano_1inidtd7j3up7os1r74ape8tjdnkgizjyfg18xmewanysk69rki41hdsjmxp"]);
}

// Set recipient camo_ addresses
function setReceiveCamoAddressList() {
    setDropdownList("receive-address-list", ["camo_1xh65p5t4amf8hcsasm7jyf1mrypkj14thneh5wxuk14c6p997wf4ihoxdhp", "camo_1inidtd7j3up7os1r74ape8tjdnkgizjyfg18xmewanysk69rki41hdsjmxp"]);
}

setDropdownList("send-address-list", ["nano_1inidtd7j3up7os1r74ape8tjdnkgizjyfg18xmewanysk69rki41hdsjmxp"]);
updateBalance("11000.31321", "9999.001");