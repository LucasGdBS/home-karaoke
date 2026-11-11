# Karaoke home

A ideia aqui é simples pórem complicada de implementar. A ideia é criar um karaoke para usar em casa, com amigos em festas. Só precisa de um notebook e dos microfones.

Um software portavel, pensei em fazer usando Taori para ficar veloz e offline-first.

Vou explicar o fluxo que estou pensando:

vc digita uma música -> usamos uma API do youtube ou um scraper não sei para pegar os primeiros resultados do youtube -> baixamos essa música localmente -> usamos uma api que eu esqueci o nome agora para buscar a letra dessa musica -> colocamos a letra no rodapé do video e fazemos um tratamento de audio para tirar o vocal.

Isso vai ficar disponivel no computador para ser executado depois.

referencia: https://www.youtube.com/watch?v=6i3vDPOjexA