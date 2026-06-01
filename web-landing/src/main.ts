import './styles/colors_and_type.css';
import './styles/landing.css';
import { mount } from 'svelte';
import App from './App.svelte';

const target: HTMLElement = document.getElementById('app')!;

const app = mount(App, { target });

export default app;
