<script>
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event'
	import ThemeSelect from '$lib/theme-select.svelte';
	import { writable } from 'svelte/store';
	import { onMount } from 'svelte';

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
		if (Array.isArray(event.payload) && event.payload.length === 2) {
			tg_folder_path = event.payload[0];
			wav_folder_path = event.payload[1];
		} else {
			console.error('Invalid payload for sync_folder_state event');
		}
	});

	let rules = writable([{ rule_name: "aaa", search_terms: [], replace_options: [] }, { rule_name: "bbb", search_terms: [], replace_options: [] }]);
	let selectedRule = $state(-1);
	let selectedTerm = $state(-1);
	let selectedRepl = $state(-1);
	let currentRuleName = $state("");
	let currentTerm = $state("");
	let currentReplPh = $state("");

    let editingRuleIndex = writable(-1);
    function handleRuleDoubleClick(index) {
        editingRuleIndex.set(index);
    }
    function handleRuleBlur(event) {
        if (event.type === 'blur' || (event.type === 'keydown' && event.key === 'Enter')) {
			console.log(event.target.value.trim());
			invoke('rename_rule', { ruleIndex: $editingRuleIndex, ruleName: event.target.value.trim() });
            editingRuleIndex.set(-1);
        }
		return true;
    }

	let editingTermIndex = writable(-1);
	function handleTermDoubleClick(index) {
		editingTermIndex.set(index);
	}
	function handleTermBlur(event) {
		if (event.type === 'blur' || (event.type === 'keydown' && event.key === 'Enter')) {
			console.log(event.target.value.trim());
			invoke('rename_find_phoneme', { ruleIndex: $selectedRule, wordIndex: $editingTermIndex, newWord: event.target.value.trim() });
			editingTermIndex.set(-1);
		}
		return true;
	}

	listen('sync_app_state', (event) => {
		console.log(event.payload);
		if (event.payload !== null && event.payload !== undefined) {
			if (Array.isArray(event.payload.rules)) {
				rules.update(_ => event.payload.rules)
			}
			selectedRule = event.payload.active_rule !== null ? event.payload.active_rule : -1;
			selectedTerm = event.payload.selected_word !== null ? event.payload.selected_word : -1;
			selectedRepl = event.payload.selected_replacement !== null ? event.payload.selected_replacement : -1;
		}
	})

	listen('sync_app_selection_state', (event) => {
		console.log(event.payload);
		if (event.payload !== null && event.payload !== undefined) {
			selectedRule = event.payload[0] !== null ? event.payload[0] : -1;
			selectedTerm = event.payload[1] !== null ? event.payload[1] : -1;
			selectedRepl = event.payload[2] !== null ? event.payload[2] : -1;
		}
	})

	function createAutoFocus(el) {
		el.focus();
		el.select();
	}

	onMount(async () => {
		let payload = await invoke('get_config_state');
		rules.update(_ => payload.rules)
		selectedRule = payload.active_rule !== null ? payload.active_rule : -1;
		selectedTerm = payload.selected_word !== null ? payload.selected_word : -1;
		selectedRepl = payload.selected_replacement !== null ? payload.selected_replacement : -1;
	});
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
			<button class="btn btn-primary flex-initial w-12" onclick={() => { invoke('add_rule', { ruleName: currentRuleName }); currentRuleName = ""; }}>Add</button>
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
							<input type="text" id={"ruleIdx"+ruleIndex} value={rule.rule_name} onkeydown={(e) => handleRuleBlur(e)} onblur={(e) => handleRuleBlur(e)} use:createAutoFocus class="input input-sm w-full transition-all" />
						{:else}
							<input type="radio" name="rule-selection" class="btn btn-sm btn-block btn-ghost justify-start transition-all" bind:group="{selectedRule}" aria-label={rule.rule_name} value={ruleIndex}  onclick={() => invoke('select_rule', { ruleIndex })}/>
						{/if}
						</div>
						<button class="hidden group-hover:inline-flex btn btn-ghost btn-circle btn-xs self-center" onclick={(e) => { invoke('remove_rule', { ruleIndex }) }}>✕</button>
					</div>
				</li>
			{/each}
			</ul>
			<ul
				tabindex="-1"
				class="shadow bg-base-200 rounded-box min-h-12 flex-1 px-2 py-2"
			>
			{#if $selectedRule > -1}
			{#each $rules[selectedRule].search_terms as term, termIndex}
				<li class="group h-8">
					<div class="flex h-full pl-0">
						<div class="flex-1 transition-all" tabindex="-1" role="button" ondblclick={() => handleTermDoubleClick(termIndex)}>
						{#if $editingTermIndex === termIndex}
							<input type="text" id={"termIdx"+termIndex} value={term} onkeydown={(e) => handleTermBlur(e)} onblur={(e) => handleTermBlur(e)} use:createAutoFocus class="input input-sm w-full transition-all" />
						{:else}
							<input type="radio" name="term-selection" class="btn btn-sm btn-block btn-ghost justify-start transition-all" bind:group="{selectedRule}" aria-label={rule.rule_name} value={ruleIndex}  onclick={() => invoke('select_find_phoneme', { ruleIndex: selectedRule, wordIndex: termIndex })}/>
						{/if}
						</div>
						<button class="hidden group-hover:inline-flex btn btn-ghost btn-circle btn-xs self-center" onclick={(e) => { invoke('remove_rule', { ruleIndex }) }}>✕</button>
					</div>
				</li>
			{/each}
			{/if}
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
