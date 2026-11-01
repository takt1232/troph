<script lang="ts">
  import { onMount } from 'svelte';
  import Icon from './Icon.svelte';

  let {
    onSubmit,
    error,
    busy,
  }: {
    onSubmit: (pin: string) => Promise<void>;
    error: string;
    busy: boolean;
  } = $props();

  let pin = $state('');
  let input: HTMLInputElement | undefined = $state();
  let localError = $state('');

  onMount(() => {
    input?.focus();
  });

  async function submit() {
    if (busy) return;
    if (pin.length < 4) {
      localError = 'Enter your PIN';
      return;
    }
    localError = '';
    try {
      await onSubmit(pin);
    } catch (err) {
      localError = err instanceof Error ? err.message : 'Login failed';
      pin = '';
      input?.focus();
    }
  }
</script>

<div class="login">
  <form
    class="login-form"
    onsubmit={(event) => {
      event.preventDefault();
      void submit();
    }}
  >
    <div class="login-mark"><Icon name="server" size={30} /></div>
    <h1>Troph</h1>
    <p>Enter the PIN shown on your phone to open your files.</p>
    <input
      bind:this={input}
      bind:value={pin}
      class="pin-input"
      type="password"
      inputmode="numeric"
      autocomplete="one-time-code"
      maxlength="12"
      placeholder="••••••"
      aria-label="PIN"
      oninput={() => {
        pin = pin.replace(/\D/g, '');
        localError = '';
      }}
    />
    <div class="login-error">{localError || error}</div>
    <button class="button primary" type="submit" disabled={busy}>
      {#if busy}<span class="spinner"></span>{:else}<Icon name="lock" size={17} />{/if}
      {busy ? 'Opening…' : 'Unlock'}
    </button>
  </form>
</div>
