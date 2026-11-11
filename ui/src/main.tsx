import React from 'react';
import ReactDOM from 'react-dom/client';
import { CubePage } from './map/CubePage';
import { MapPage } from './map/MapPage';
import './style.css';

const smoke = new URLSearchParams(window.location.search).get('smoke') === '1';

ReactDOM.createRoot(document.getElementById('root')!).render(
  <React.StrictMode>{smoke ? <CubePage /> : <MapPage />}</React.StrictMode>,
);
