<script>
    import { invoke } from '@tauri-apps/api/core';
    import { listen } from '@tauri-apps/api/event'
	import { themes } from '$lib/themes';
	import { writable } from 'svelte/store';
	import { onMount } from 'svelte';

	let isDropdownOpen = $state(false);

	let tg_folder_path = $state('No folder selected.');
	let wav_folder_path = $state('No folder selected.');

	let rules = writable([]);
	let items = writable([]);
	let selectedRuleIdx = $state(-1);
	let selectedTermIdx = $state(-1);
	let selectedOptIdx = $state(-1);
	let selectedItemIdx = $state(-1);
	let selectedMarkIdx = $state(-1);

	let currentRuleName = $state("");
	let currentTerm = $state("");
	let currentOpt = $state("");
	let optButtonDisabled = $state(false);

	let currentTheme = $state("");
	let soundDevice = $state("");
	let defaultSoundDevice = $state("");
	let soundDevices = writable([]);
	let volumeFactor = $state(1.0);
	let autoBackup = $state(true);
	let autoNext = $state(true);
	let autoPlay = $state(true);
	let autoScroll = $state(true);

	let ruleDiv = null;
	let termDiv = null;
	let optDiv = null;
	let itemDiv = null;
	let markDiv = null;

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

    let editingRuleIndex = writable(-1);
    function handleRuleDoubleClick(index) {
        editingRuleIndex.set(index);
    }
    function handleRuleBlur(event) {
        if (event.type === 'blur' || (event.type === 'keydown' && event.key === 'Enter')) {
			if (event.target.value.trim().length > 0) {
				invoke('rename_rule', { ruleIndex: $editingRuleIndex, newName: event.target.value.trim() });
			}
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
			if (event.target.value.trim().length > 0) {
				invoke('rename_search_term', { termIndex: $editingTermIndex, newTerm: event.target.value.trim() });
			}
			editingTermIndex.set(-1);
		}
		return true;
	}

	let editingOptIndex = writable(-1);
	function handleOptDoubleClick(index) {
		editingOptIndex.set(index);
	}
	function handleOptBlur(event) {
		if (event.type === 'blur' || (event.type === 'keydown' && event.key === 'Enter')) {
			if (event.target.value.trim().length > 0) {
				invoke('rename_replace_option', { optIndex: $editingOptIndex, newOpt: event.target.value.trim() });
			}
			editingOptIndex.set(-1);
		}
		return true;
	}

	function createAutoFocus(el) {
		el.focus();
		el.select();
	}

	function setTheme(event) {
        const theme = event.target.value;
        if (themes.includes(theme)) {
			invoke('update_settings', {theme: currentTheme, volumeFactor, autoBackup, autoNext, autoPlay, autoScroll});
        }
    }

	function nextTheme() {
		const index = themes.indexOf(currentTheme);
		invoke('update_settings', {theme: themes[(index + 1) % themes.length], volumeFactor, autoBackup, autoNext, autoPlay, autoScroll});
	}

	function prevTheme() {
		const index = themes.indexOf(currentTheme);
		invoke('update_settings', {theme: themes[(index - 1 + themes.length) % themes.length], volumeFactor, autoBackup, autoNext, autoPlay, autoScroll});
	}

	function selectItem(itemIndex) {
		invoke('select_item', { itemIndex });
	}

	function selectMark(markIndex) {
		invoke('select_mark', { markIndex });
		if (autoPlay) {
			invoke('play_selected');
		}
	}

	function recenter() {
		if (selectedItemIdx > -1) {
			scrollIntoCenterView(selectedItemIdx, itemDiv);
			if (selectedMarkIdx > -1) {
				scrollIntoCenterView(selectedMarkIdx, markDiv);
			}
		}
	}

	function scrollIntoCenterView(idx, parent) {
		if (idx < 0 || !parent) return;

		const parentRect = parent.getBoundingClientRect();
		const scrollTop = idx * 32 - parentRect.height / 2 + 16;
		parent.scrollTo({ top: scrollTop, behavior: 'smooth', block: 'center' });
	}

	listen('sync_folder_state', (event) => {
		if (Array.isArray(event.payload) && event.payload.length === 2) {
			tg_folder_path = event.payload[0];
			wav_folder_path = event.payload[1];
		} else {
			console.error('Invalid payload for sync_folder_state event');
		}
	});

	listen('sync_app_state', (event) => {
		console.log(event.payload);
		if (event.payload !== null && event.payload !== undefined) {
			if (Array.isArray(event.payload.rules)) {
				rules.update(_ => event.payload.rules)
			}
			tg_folder_path = event.payload.tg_folder !== null ? event.payload.tg_folder : "No Folder Selected.";
			wav_folder_path = event.payload.wav_folder !== null ? event.payload.wav_folder : "No Folder Selected.";
			selectedRuleIdx = event.payload.selected_rule_idx !== null ? event.payload.selected_rule_idx : -1;
			selectedTermIdx = event.payload.selected_term_idx !== null ? event.payload.selected_term_idx : -1;
			selectedOptIdx = event.payload.selected_opt_idx !== null ? event.payload.selected_opt_idx : -1;
		}
	})

	listen('sync_session_state', (event) => {
		console.log(event.payload);
		if (event.payload !== null && event.payload !== undefined) {
			items.update(_ => event.payload.items)
			selectedItemIdx = event.payload.selected_item !== null ? event.payload.selected_item : -1;
			if (selectedItemIdx >= 0) {
				selectedMarkIdx = event.payload.selected_mark[selectedItemIdx] !== null ? event.payload.selected_mark[selectedItemIdx] : -1;
				scrollIntoCenterView(selectedItemIdx, itemDiv);
				if (selectedMarkIdx > -1) {
					scrollIntoCenterView(selectedMarkIdx, markDiv);
				}
			}
		}
	})

	listen('sync_app_selection_state', (event) => {
		console.log('sync_app_selection_state', event.payload);
		if (event.payload !== null && event.payload !== undefined) {
			selectedRuleIdx = event.payload[0] !== null ? event.payload[0] : -1;
			selectedTermIdx = event.payload[1] !== null ? event.payload[1] : -1;
			selectedOptIdx = event.payload[2] !== null ? event.payload[2] : -1;
		}
	})

	listen('sync_item_selection_state', (event) => {
		if (event.payload !== null && event.payload !== undefined) {
			console.log('sync_item_selection_state', event.payload);
			if (event.payload[0] === null) {
				event.payload[0] = -1;
			}
			if (event.payload[1] === null) {
				event.payload[1] = -1;
			}
			if ((selectedItemIdx !== event.payload[0] || selectedMarkIdx !== event.payload[1]) && autoPlay) {
				invoke('play_selected');
			}
			selectedItemIdx = event.payload[0];
			selectedMarkIdx = event.payload[1];
			if (autoScroll && selectedItemIdx > -1) {
				scrollIntoCenterView(selectedItemIdx, itemDiv);
				if (selectedMarkIdx > -1) {
					scrollIntoCenterView(selectedMarkIdx, markDiv);
				}
			}
		}
	})

	listen('sync_settings', (event) => {
		console.log('sync_settings', event.payload);
		if (event.payload !== null && event.payload !== undefined) {
			currentTheme = event.payload.theme;
			document.documentElement.setAttribute('data-theme', currentTheme);
			if (event.payload.sound_device !== null) {
				soundDevice = event.payload.sound_device;
			}
			volumeFactor = event.payload.volume_factor;
			autoBackup = event.payload.auto_backup;
			autoNext = event.payload.auto_next;
			autoPlay = event.payload.auto_play;
			autoScroll = event.payload.auto_scroll;
		}
	})

	listen('save_textgrids_done', (event) => {
		console.log('save_textgrids_done', event.payload);
		loading_modal.close();
	})
	listen('list_item_done', (event) => {
		console.log('list_item_done', event.payload);
		loading_modal.close();
	})

	async function choose_a_replace_option(optIndex) {
		optButtonDisabled = true;
		let payload = await invoke('choose_a_replace_option', { optIndex });
		if (payload !== null) {
			items.update(items => {
				items[selectedItemIdx].selected_options = payload[0];
				items[selectedItemIdx].dirty = payload[1];
				return items
			})
		}
		if (autoNext) {
			await invoke('next_mark');
		}
		optButtonDisabled = false;
	}

	onMount(async () => {
		let config_state = await invoke('get_config_state');
		rules.update(_ => config_state.rules)
		tg_folder_path = config_state.tg_folder !== null ? config_state.tg_folder : "No Folder Selected.";
		wav_folder_path = config_state.wav_folder !== null ? config_state.wav_folder : "No Folder Selected.";
		selectedRuleIdx = config_state.selected_rule_idx !== null ? config_state.selected_rule_idx : -1;
		selectedTermIdx = config_state.selected_term_idx !== null ? config_state.selected_term_idx : -1;
		selectedOptIdx = config_state.selected_opt_idx !== null ? config_state.selected_opt_idx : -1;

		let session_items = await invoke('get_session_items');
		items.update(_ => session_items.items);
		selectedItemIdx = session_items.selected_item !== null ? session_items.selected_item : -1;
		if (session_items.selected_mark !== null) {
			selectedMarkIdx = session_items.selected_mark[selectedItemIdx] !== null ? session_items.selected_mark[selectedItemIdx] : -1;
		}

		let app_settings = await invoke('get_app_settings');
		currentTheme = app_settings.theme;
		document.documentElement.setAttribute('data-theme', currentTheme);
		if (app_settings.sound_device !== null) {
			soundDevice = app_settings.sound_device;
		}
		volumeFactor = app_settings.volume_factor;
		autoBackup = app_settings.auto_backup;
		autoNext = app_settings.auto_next;
		autoPlay = app_settings.auto_play;
		autoScroll = app_settings.auto_scroll;

		let audio_devices = await invoke('list_audio_output_devices');
		if (app_settings.sound_device === null) {
			soundDevice = audio_devices[0];
		}
		defaultSoundDevice = audio_devices[0];
		soundDevices.update(_ => audio_devices[1]);

		setTimeout(() => {
			if (selectedItemIdx > -1) {
				scrollIntoCenterView(selectedItemIdx, itemDiv);
				if (selectedMarkIdx > -1) {
					scrollIntoCenterView(selectedMarkIdx, markDiv);
				}
			}
		}, 100);
	});
</script>

<svelte:head>
	<title>Label Replacer</title>
	<meta name="description" content="Label Replacer by hrukalive" />
</svelte:head>

<section class="h-screen">
	<dialog id="loading_modal" class="modal modal-bottom sm:modal-middle transition-all duration-300">
		<div class="p-4 w-32 rounded-box shadow bg-white flex justify-center">
			<div class="loading loading-infinity loading-lg place-self-center"></div>
		</div>
	</dialog>
	<dialog id="setting_modal" class="modal modal-bottom sm:modal-middle">
		<div class="modal-box">
			<h1 class="font-bold text-2xl">Settings</h1>
			<div class="mt-6 flex flex-col gap-4">
				<div class="grid grid-cols-5">
					<span class="label">Theme</span>
					<select
						bind:value={currentTheme}
						data-choose-theme
						class="col-span-3 select select-bordered select-primary w-full text-sm capitalize"
						onchange={(e) => setTheme(e)}
					>
						{#each themes as theme}
							<option value={theme} class="capitalize">{theme}</option>
						{/each}
					</select>
					<div class="join ml-2 self-center place-self-center justify-center">
						<button class="join-item btn" onclick={prevTheme}>«</button>
						<button class="join-item btn" onclick={nextTheme}>»</button>
					</div>
				</div>
				<div class="grid grid-cols-5">
					<span class="label">Device</span>
					<select
						bind:value={soundDevice}
						class="col-span-3 select select-bordered select-primary w-full text-sm capitalize"
						onchange={(e) => invoke('select_audio_output_device', { deviceName: e.target.value })}
					>
						{#each $soundDevices as device}
							<option value={device} class="capitalize">{(device === defaultSoundDevice ? '(Default) ' : '') + device}</option>
						{/each}
					</select>
					<button class="ml-2 btn" onclick={() => invoke('test_output_device')}>Test</button>
				</div>
				<div class="grid grid-cols-5 justify-center">
					<span class="label">Volume</span>
					<input
						type="range"
						min="0.1"
						max="2.0"
						step="0.1"
						bind:value={volumeFactor}
						class="col-span-3 range range-sm self-center"
						onmouseup={() => invoke('update_settings', {theme: currentTheme, volumeFactor, autoBackup, autoNext, autoPlay, autoScroll})}
					/>
					<div class="badge badge-outline place-self-center">{volumeFactor.toFixed(1)}</div>
				</div>
				<div class="grid grid-cols-10">
					<span class="label col-span-3">Auto-backup</span>
					<input
						type="checkbox"
						bind:checked={autoBackup}
						class="col-span-2 toggle toggle-primary place-self-center justify-self-start"
						onchange={() => invoke('update_settings', {theme: currentTheme, volumeFactor, autoBackup, autoNext, autoPlay, autoScroll})}
					/>
					<span class="label col-start-7 col-span-2">Auto-next</span>
					<input
						type="checkbox"
						bind:checked={autoNext}
						class="col-span-2 toggle toggle-primary place-self-center"
						onchange={() => invoke('update_settings', {theme: currentTheme, volumeFactor, autoBackup, autoNext, autoPlay, autoScroll})}
					/>
				</div>
				<div class="grid grid-cols-10">
					<span class="label col-span-3">Auto-play</span>
					<input
						type="checkbox"
						bind:checked={autoPlay}
						class="col-span-2 toggle toggle-primary place-self-center justify-self-start"
						onchange={() => invoke('update_settings', {theme: currentTheme, volumeFactor, autoBackup, autoNext, autoPlay, autoScroll})}
					/>
					<span class="label col-start-7 col-span-2">Auto-scroll</span>
					<input
						type="checkbox"
						bind:checked={autoScroll}
						class="col-span-2 toggle toggle-primary place-self-center"
						onchange={() => invoke('update_settings', {theme: currentTheme, volumeFactor, autoBackup, autoNext, autoPlay, autoScroll})}
					/>
				</div>
			</div>
			<div class="modal-action">
				<form method="dialog">
					<button class="btn">Done</button>
				</form>
			</div>
		</div>
		<form method="dialog" class="modal-backdrop">
			<button class="cursor-default">close</button>
		</form>
	</dialog>
	<div class="flex flex-col h-full">
		<div>
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
							<li><a onclick={() => invoke('init_state')}>New project</a></li>
							<li><a onclick={() => invoke('load_state')}>Open project</a></li>
							<li><a onclick={() => invoke('save_state')}>Save project</a></li>
						</ul>
					</div>
				</div>
				<div class="navbar-center">
					<h1 class="font-bold text-2xl">Label Replacer</h1>
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
		</div>
		<div class="flex-grow p-4">
			<div class="flex flex-col space-y-2 h-full">
				<div class="flex w-full justify-center">
					<button class="btn btn-outline flex-initial w-48" onclick={() => invoke('open_folder', { target: 'tg' })}>Open TextGrid Folder</button>
					<span id="tg-folder-text" class="font-normal h-8 m-2 mx-2 px-2 leading-7 border-b-2 flex-1"
						>{ tg_folder_path }</span
					>
				</div>
				<div class="flex w-full justify-center">
					<button class="btn btn-outline flex-initial w-48" onclick={() => invoke('open_folder', { target: 'wav' })}>Open WAV Folder</button>
					<span id="wav-folder-text" class="font-normal h-8 m-2 mx-2 px-2 leading-7 border-b-2 flex-1"
						>{ wav_folder_path }</span
					>
				</div>
				<div class="flex w-full justify-center space-x-4">
					<input id="rule-name-input" bind:value={currentRuleName} type="text" placeholder="Rule name" class="input input-sm input-bordered flex-1" onkeydown={(e) => { if (e.key === "Enter") { invoke('add_rule', { ruleName: currentRuleName.trim() }); currentRuleName = ""; document.getElementById("rule-name-input").focus(); } }} />
					<button class="btn btn-sm flex-initial w-8" onclick={() => { invoke('add_rule', { ruleName: currentRuleName.trim() }); currentRuleName = ""; document.getElementById("rule-name-input").focus(); } }>+</button>
					<input id="search-term-input" bind:value={currentTerm} type="text" placeholder="Find Phoneme" class="input input-sm input-bordered flex-1" onkeydown={(e) => { if (e.key === "Enter") { invoke('add_search_term', { term: currentTerm.trim() }); currentTerm = ""; document.getElementById("search-term-input").focus(); } }} />
					<button class="btn btn-sm flex-initial w-8" onclick={() => { invoke('add_search_term', { term: currentTerm.trim() }); currentTerm = ""; document.getElementById("search-term-input").focus(); } }>+</button>
					<input id="replace-opt-input" bind:value={currentOpt} type="text" placeholder="Replace Phoneme" class="input input-sm input-bordered flex-1" onkeydown={(e) => { if (e.key === "Enter") { invoke('add_replace_option', { replaceOpt: currentOpt.trim() }); currentOpt = ""; document.getElementById("replace-opt-input").focus(); } }} />
					<button class="btn btn-sm flex-initial w-8" onclick={() => { invoke('add_replace_option', { replaceOpt: currentOpt.trim() }); currentOpt = ""; document.getElementById("replace-opt-input").focus(); }}>+</button>
				</div>
				<div class="flex w-full justify-center space-x-4">
					<ul
						tabindex="-1"
						class="shadow bg-base-200 rounded-box min-h-12 flex-1 px-2 py-2"
					>
						<div class="overflow-y-auto overflow-x-hidden min-h-24 max-h-24">
							{#each $rules as rule, ruleIndex}
								<li class="group h-8">
									<div class="flex h-full pl-0">
										<div class="flex-1 transition-all" tabindex="-1" role="button" ondblclick={() => handleRuleDoubleClick(ruleIndex)}>
										{#if $editingRuleIndex === ruleIndex}
											<input type="text" id={"ruleIdx"+ruleIndex} value={rule.rule_name} onkeydown={(e) => handleRuleBlur(e)} onblur={(e) => handleRuleBlur(e)} use:createAutoFocus class="input input-sm w-full transition-all" />
										{:else}
											<input type="radio" name="rule-selection" class="btn btn-sm btn-block btn-ghost justify-start transition-all" checked={selectedRuleIdx === ruleIndex} aria-label={rule.rule_name} value={ruleIndex}  onclick={() => invoke('select_rule', { ruleIndex })}/>
										{/if}
										</div>
										<button class="hidden group-hover:inline-flex btn btn-ghost btn-circle btn-xs self-center" onclick={(e) => { invoke('remove_rule', { ruleIndex }) }}>✕</button>
									</div>
								</li>
							{/each}
						</div>
					</ul>
					<ul
						tabindex="-1"
						class="shadow bg-base-200 rounded-box min-h-12 flex-1 px-2 py-2"
					>
						<div class="overflow-y-auto overflow-x-hidden min-h-24 max-h-24">
							{#if selectedRuleIdx > -1}
								{#each $rules[selectedRuleIdx].search_terms as term, termIndex}
									<li class="group h-8">
										<div class="flex h-full pl-0">
											<div class="flex-1 transition-all" tabindex="-1" role="button" ondblclick={() => handleTermDoubleClick(termIndex)}>
											{#if $editingTermIndex === termIndex}
												<input type="text" id={"termIdx"+termIndex} value={term} onkeydown={(e) => handleTermBlur(e)} onblur={(e) => handleTermBlur(e)} use:createAutoFocus class="input input-sm w-full transition-all" />
											{:else}
												<input type="radio" name="term-selection" class="btn btn-sm btn-block btn-ghost justify-start transition-all" checked={selectedTermIdx === termIndex} aria-label={term} value={termIndex}  onclick={() => invoke('select_search_term', { termIndex })}/>
											{/if}
											</div>
											<button class="hidden group-hover:inline-flex btn btn-ghost btn-circle btn-xs self-center" onclick={(e) => { invoke('remove_search_term', { termIndex }) }}>✕</button>
										</div>
									</li>
								{/each}
							{/if}
						</div>
					</ul>
					<ul
						tabindex="-1"
						class="shadow bg-base-200 rounded-box min-h-12 flex-1 px-2 py-2"
					>
						<div class="overflow-y-auto overflow-x-hidden min-h-24 max-h-24">
							{#if selectedRuleIdx > -1}
								{#each $rules[selectedRuleIdx].replace_options as opt, optIndex}
									<li class="group h-8">
										<div class="flex h-full pl-0">
											<div class="flex-1 transition-all" tabindex="-1" role="button" ondblclick={() => handleOptDoubleClick(optIndex)}>
											{#if $editingOptIndex === optIndex}
												<input type="text" id={"optIdx"+optIndex} value={opt} onkeydown={(e) => handleOptBlur(e)} onblur={(e) => handleOptBlur(e)} use:createAutoFocus class="input input-sm w-full transition-all" />
											{:else}
												<input type="radio" name="opt-selection" class="btn btn-sm btn-block btn-ghost justify-start transition-all" checked={selectedOptIdx === optIndex} aria-label={opt} value={optIndex} onclick={() => invoke('select_replace_option', { optIndex })}/>
											{/if}
											</div>
											<button class="hidden group-hover:inline-flex btn btn-ghost btn-circle btn-xs self-center" onclick={(e) => { invoke('remove_replace_option', { optIndex }) }}>✕</button>
										</div>
									</li>
								{/each}
							{/if}
						</div>
					</ul>
				</div>
				<div class="flex w-full justify-center">
					<button class="btn btn-secondary flex-1" onclick={() => { loading_modal.showModal(); invoke('list_items', { target: 'wav' }); }}>List</button>
				</div>
				<div class="flex w-full justify-center">
					<div class="grid grid-cols-8 gap-2 w-full justify-center">
						<div class="col-span-2 gap-2 flex">
							<button class="flex-1 btn btn-neutral btn-sm" onclick={() => invoke('prev_item')}>Previous</button>
							<button class="flex-1 btn btn-neutral btn-sm" onclick={() => invoke('next_item')}>Next</button>
						</div>
						<div class="col-span-3 gap-2 flex">
							<button class="flex-1 btn btn-neutral btn-sm" onclick={() => recenter()}>Re-center</button>
							<button class="flex-1 btn btn-neutral btn-sm" onclick={() => invoke('prev_mark')}>Previous</button>
							<button class="flex-1 btn btn-neutral btn-sm" onclick={() => invoke('next_mark')}>Next</button>
						</div>
						<div class="col-span-3 gap-2 flex">
							<button class="flex-1 btn btn-neutral btn-sm" onclick={() => invoke('play_selected')}>Play</button>
							<button class="flex-1 btn btn-neutral btn-sm" onclick={() => { loading_modal.showModal(); invoke('save_textgrids'); }}>Save all</button>
						</div>
					</div>
				</div>
				<div class="flex-grow grid grid-cols-8 gap-2 w-full justify-center">
					<ul
						id="item-ul"
						tabindex="-1"
						class="shadow bg-base-200 rounded-box min-h-12 col-span-2 h-auto px-2 py-2"
					>
						<div id="item-list" bind:this={itemDiv} class="overflow-y-auto overflow-x-hidden max-h-[calc(100vh-480px)]">
							{#each $items as item, itemIndex}
								<li class="group h-8">
									<div class="flex h-full pl-0">
										<input type="radio" name="item-selection" class="flex-1 btn btn-sm btn-block btn-ghost justify-start transition-all" checked={selectedItemIdx === itemIndex} aria-label={`${item.dirty?"*":""}${item.tg_stem}`} value={itemIndex}  onclick={() => selectItem(itemIndex)}/>
									</div>
								</li>
							{/each}
						</div>
					</ul>
					<ul
						id="mark-ul"
						tabindex="-1"
						class="shadow bg-base-200 rounded-box min-h-12 col-span-3 h-auto px-2 py-2"
					>
						<div id="mark-list" bind:this={markDiv} class="overflow-y-auto overflow-x-hidden max-h-[calc(100vh-480px)]">
							{#if selectedItemIdx > -1}
								{#each $items[selectedItemIdx].found_mark_titles as mark, markIndex}
									<li class="group h-8">
										<div class="flex h-full pl-0">
											<input type="radio" name="mark-selection" class="flex-1 btn btn-sm btn-block btn-ghost justify-start transition-all" checked={selectedMarkIdx === markIndex} aria-label={mark} value={markIndex}  onclick={() => selectMark(markIndex)}/>
										</div>
									</li>
								{/each}
							{/if}
						</div>
					</ul>
					<div class="flex bg-base-200 rounded-box col-span-3 h-auto justify-center items-center">
						<div class="flex flex-wrap gap-2 justify-center items-center transition-all ease-in-out">
							{#if selectedItemIdx > -1 && selectedMarkIdx > -1 && $items[selectedItemIdx].selected_options.length > 0}
								<button class="btn {$items[selectedItemIdx].selected_options[selectedMarkIdx] === null ? 'btn-accent btn-lg' : 'btn-primary btn-md'} transition-all ease-in-out" disabled={optButtonDisabled} onclick={() => { choose_a_replace_option(-1) } }>✅</button>
								{#each $items[selectedItemIdx].replace_options as opt, optIndex}
									<button class="btn {$items[selectedItemIdx].selected_options[selectedMarkIdx] === optIndex ? 'btn-accent btn-lg' : 'btn-primary btn-md'} transition-all ease-in-out" disabled={optButtonDisabled} onclick={() => { choose_a_replace_option(optIndex) } }>{ opt }</button>
								{/each}
							{/if}
						</div>
					</div>
				</div>
			</div>
		</div>
	</div>
</section>
