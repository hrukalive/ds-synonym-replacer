<script>
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event'
	import ThemeSelect from '$lib/theme-select.svelte';
	import { writable } from 'svelte/store';

	let isDropdownOpen = $state(false);
	const handleDropdownClick = () => {
		isDropdownOpen = !isDropdownOpen;
	};
	const handleDropdownFocusLoss = ({ relatedTarget, currentTarget }) => {
		if (relatedTarget instanceof HTMLElement && currentTarget.contains(relatedTarget))
            return;
		isDropdownOpen = false;
	};
    const handleKeyDown = (event) => {
		if (event.key === 'Enter' || event.key === ' ') {
			handleDropdownClick();
		}
	};

	let tg_folder_path = $state('No folder selected.');
	let wav_folder_path = $state('No folder selected.');

	listen('sync_folder_state', (event) => {
		console.log(event);
		if (event.payload !== null && event.payload !== undefined) {
			if (event.payload.tg_folder !== null && event.payload.tg_folder !== undefined) {
				tg_folder_path = event.payload.tg_folder;
			} else {
				tg_folder_path = 'No folder selected.';
			}
			if (event.payload.wav_folder !== null && event.payload.wav_folder !== undefined) {
				wav_folder_path = event.payload.wav_folder;
			} else {
				wav_folder_path = 'No folder selected.';
			}
		}
	});

	let rules = writable([{ name: "aaa", searchTerms: [], replacements: [] }, { name: "bbb", searchTerms: [], replacements: [] }]);
	let selectedRule = $state("");
	let currentRuleName = $state("");
	let currentTerm = $state("");
	let currentReplPh = $state("");

    let editingRuleIndex = writable(-1);

    function handleRuleDoubleClick(index) {
        editingRuleIndex.set(index);
    }

    function handleRuleBlur(event) {
        if (event.type === 'blur' || (event.type === 'keydown' && event.key === 'Enter')) {
            editingRuleIndex.set(-1);
        }
		return true;
    }

    function addRule(name) {
		name = name.trim();
		if (name === "") {
			return;
		}
		// find existing rule index
		const existingRule = $rules.find(rule => rule.name === name);
		if (existingRule) {
			selectedRule = existingRule.name;
			return;
		}
        rules.update(rules => [...rules, { name, searchTerms: [], replacements: [] }]);
    }

	function removeRule(index) {
		rules.update(rules => {
			rules.splice(index, 1);
			return rules;
		});
	}

	function selectRule(ruleName) {
		selectedRule = ruleName;
		console.log(selectedRule);
	}

	function updateRuleName(index, event) {
        rules.update(rules => {
            rules[index].name = event.target.value.trim();
            return rules;
        });
		selectRule = rules[index].name;
    }

	function createAutoFocus(el) {
		el.focus();
		el.select();
	}

    // function addSearchTerm(ruleIndex, ph) {
    //     rules.update(rules => {
    //         rules[ruleIndex].searchTerms.push(ph);
    //         return rules;
    //     });
    // }

    // function addReplacement(ruleIndex, ph) {
    //     rules.update(rules => {
    //         rules[ruleIndex].replacements.push(ph);
    //         return rules;
    //     });
    // }

    // function updateItem(ruleIndex, list, itemIndex, value) {
    //     rules.update(rules => {
    //         rules[ruleIndex][list][itemIndex] = value;
    //         return rules;
    //     });
    // }

    // function deleteItem(ruleIndex, list, itemIndex) {
    //     rules.update(rules => {
    //         rules[ruleIndex][list].splice(itemIndex, 1);
    //         return rules;
    //     });
    // }
</script>

<svelte:head>
	<title>Home</title>
	<meta name="description" content="Svelte demo app" />
</svelte:head>

<section>
	<dialog id="setting_modal" class="modal modal-bottom sm:modal-middle">
		<div class="modal-box">
			<!-- <form method="dialog">
                <button class="btn btn-sm btn-circle btn-ghost absolute right-2 top-2">✕</button>
            </form> -->
			<h1 class="font-bold text-2xl">Settings</h1>
			<div class="mt-6">
				<ThemeSelect />
				<button class="btn">neutral</button>
				<button class="btn btn-primary">primary</button>
				<button class="btn btn-secondary">secondary</button>
				<button class="btn btn-accent">accent</button>
				<button class="btn btn-ghost">ghost</button>
				<button class="btn btn-link">link</button>
			</div>
			<div class="modal-action">
				<form method="dialog">
					<button class="btn">Done</button>
				</form>
			</div>
		</div>
	</dialog>
	<div class="navbar bg-base-200">
		<div class="navbar-start">
			<div class="dropdown" onfocusout={handleDropdownFocusLoss}>
				<div
					tabindex="0"
					role="button"
					class="btn btn-ghost btn-circle"
					onclick={handleDropdownClick}
                    onkeydown={handleKeyDown}
				>
					{#if isDropdownOpen}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							fill="none"
							viewBox="0 0 24 24"
							class="inline-block h-6 w-6 stroke-current"
						>
							<title>Close Dropdown</title>
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M6 18L18 6M6 6l12 12"
							/>
						</svg>
					{:else}
						<svg
							xmlns="http://www.w3.org/2000/svg"
							fill="none"
							viewBox="0 0 24 24"
							class="inline-block h-6 w-6 stroke-current"
						>
							<title>Open Dropdown</title>
							<path
								stroke-linecap="round"
								stroke-linejoin="round"
								stroke-width="2"
								d="M4 6h16M4 12h16M4 18h16"
							/>
						</svg>
					{/if}
				</div>
				<ul
					tabindex="-1"
					class="menu menu-sm dropdown-content mt-3 z-[1] p-2 shadow bg-base-300 rounded-box w-52 transition-opacity"
                    style:visibility={isDropdownOpen ? 'visible' : 'hidden'}
				>
					<li><a>Open project</a></li>
					<li><a>Save project</a></li>
				</ul>
			</div>
		</div>
		<div class="navbar-center">
			<a class="btn btn-ghost text-xl">Label Replacer</a>
		</div>
		<div class="navbar-end">
			<button class="btn btn-square btn-ghost" onclick={() => setting_modal.showModal()}>
				<svg
					xmlns="http://www.w3.org/2000/svg"
					fill="none"
					viewBox="0 0 24 24"
					class="inline-block w-5 h-5 stroke-current"
					><path
						stroke-linecap="round"
						stroke-linejoin="round"
						stroke-width="2"
						d="M5 12h.01M12 12h.01M19 12h.01M6 12a1 1 0 11-2 0 1 1 0 012 0zm7 0a1 1 0 11-2 0 1 1 0 012 0zm7 0a1 1 0 11-2 0 1 1 0 012 0z"
					></path></svg
				>
			</button>
		</div>
	</div>
	<div class="mx-auto pt-4 space-y-2 px-4">
		<div class="flex w-full justify-center">
			<button class="btn btn-primary flex-initial w-48" onclick={() => invoke('open_folder', { target: 'tg' })}>Open TextGrid Folder</button>
			<span id="tg-folder-text" class="font-normal h-8 m-2 mx-2 px-2 leading-7 border-b-2 flex-1"
				>{ tg_folder_path }</span
			>
		</div>
		<div class="flex w-full justify-center">
			<button class="btn btn-primary flex-initial w-48" onclick={() => invoke('open_folder', { target: 'wav' })}>Open WAV Folder</button>
			<span id="wav-folder-text" class="font-normal h-8 m-2 mx-2 px-2 leading-7 border-b-2 flex-1"
				>{ wav_folder_path }</span
			>
		</div>
		<div class="flex w-full justify-center space-x-4">
			<input bind:value={currentRuleName} type="text" placeholder="Rule name" class="input input-bordered flex-1" />
			<button class="btn btn-primary flex-initial w-12" onclick={() => { addRule(currentRuleName); currentRuleName = ""; }}>Add</button>
			<input bind:value={currentTerm} type="text" placeholder="Find Phoneme" class="input input-bordered flex-1" />
			<button class="btn btn-primary flex-initial w-12" onclick={() => { selectRule("aaa"); }}>Add</button>
			<input bind:value={currentReplPh} type="text" placeholder="Replace Phoneme" class="input input-bordered flex-1" />
			<button class="btn btn-primary flex-initial w-12" onclick={() => {}}>Add</button>
		</div>
		<div class="flex w-full justify-center space-x-4">
			<ul
				tabindex="-1"
				class="shadow bg-base-200 rounded-box min-h-12 flex-1 px-2 py-2"
			>
			{#each $rules as rule, ruleIndex}
				<li class="group h-8">
					<div class="flex h-full pl-0">
						<div class="flex-1 transition-all" tabindex="-1" role="button" ondblclick={() => handleRuleDoubleClick(ruleIndex)}>
						{#if $editingRuleIndex === ruleIndex}
							<input type="text" id={"ruleIdx"+ruleIndex} bind:value={rule.name} onkeydown={(e) => handleRuleBlur(e)} onblur={(e) => handleRuleBlur(e)} oninput={(e) => updateRuleName(ruleIndex, e)} use:createAutoFocus class="input input-sm w-full transition-all" />
						{:else}
							<input type="radio" name="rule-selection" class="btn btn-sm btn-block btn-ghost justify-start transition-all" bind:group="{selectedRule}" aria-label={rule.name} value={rule.name}  onclick={() => selectRule(rule.name)}/>
						{/if}
						</div>
						<button class="hidden group-hover:inline-flex btn btn-ghost btn-circle btn-xs self-center" onclick={(e) => { e.preventDefault(); removeRule(ruleIndex) }}>✕</button>
					</div>
				</li>
			{/each}
			</ul>
			<!-- <ul
				tabindex="-1"
				class="menu menu-sm dropdown-content shadow bg-base-200 rounded-box min-h-12 flex-1"
			>
				<li><a>Placeholder</a></li>
				<li><a>Placeholder</a></li>
			</ul>
			<ul
				tabindex="-1"
				class="menu menu-sm dropdown-content shadow bg-base-200 rounded-box min-h-12 flex-1"
			>
				<li><a>Placeholder</a></li>
				<li><a>Placeholder</a></li>
			</ul> -->
			<!-- <div class="flex w-full justify-center space-x-4">
				<button class="btn btn-primary" onclick={addRule}>Add Rule</button>
			</div> -->
			<!-- {#each $rules as rule, ruleIndex}
				<div class="flex w-full justify-center space-x-4 mt-4">
					<input type="text" placeholder="Rule name" bind:value={rule.name} class="input input-bordered flex-1" />
					<button class="btn btn-primary" onclick={() => addSearchTerm(ruleIndex)}>Add Search Term</button>
					<button class="btn btn-primary" onclick={() => addReplacement(ruleIndex)}>Add Replacement</button>
				</div>
				<div class="flex w-full justify-center space-x-4 mt-2">
					<ul class="menu menu-sm dropdown-content shadow bg-base-200 rounded-box min-h-12 flex-1">
						{#each rule.searchTerms as term, termIndex}
							<li>
								<div class="flex justify-between items-center">
									<input type="text" bind:value={rules[ruleIndex][termIndex]} class="input input-bordered flex-1" ondblclick={() => updateItem(ruleIndex, 'searchTerms', termIndex, prompt('Edit search term', term))} />
									<button class="btn btn-ghost" onclick={() => deleteItem(ruleIndex, 'searchTerms', termIndex)}>✕</button>
								</div>
							</li>
						{/each}
					</ul>
					<ul class="menu menu-sm dropdown-content shadow bg-base-200 rounded-box min-h-12 flex-1">
						{#each rule.replacements as replacement, replacementIndex}
							<li>
								<div class="flex justify-between items-center">
									<input type="text" bind:value={rules[ruleIndex][replacementIndex]} class="input input-bordered flex-1" ondblclick={() => updateItem(ruleIndex, 'replacements', replacementIndex, prompt('Edit replacement', replacement))} />
									<button class="btn btn-ghost" onclick={() => deleteItem(ruleIndex, 'replacements', replacementIndex)}>✕</button>
								</div>
							</li>
						{/each}
					</ul>
				</div>
			{/each} -->
		</div>
		<div class="flex w-full justify-center">
			<button class="btn btn-secondary flex-1" onclick={() => invoke('list_items', { target: 'wav' })}>List</button>
		</div>
		<div class="flex w-full justify-center">
			<button class="btn btn-secondary flex-1" onclick={() => invoke('play_test')}>Play</button>
		</div>
	</div>
</section>
