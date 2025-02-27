import { iso } from '@iso';
import { CardMedia } from '@mui/material';
import React from 'react';

export const ImageDisplay = iso(`
  field Image.ImageDisplay @component {
    url
  }
`)(({ data: image }) => (
  <CardMedia
    component="img"
    image={image.url}
    height="194"
    sx={{ objectFit: 'contain' }}
  />
));
